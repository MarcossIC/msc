use crate::core::system_info::types::{
    BluetoothAdapter, BluetoothVersion, EthernetAdapter, NetworkInfo, WifiAdapter, WifiBand,
    WifiStandard,
};
use crate::error::{MscError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use wmi::WMIConnection;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Win32NetworkAdapter {
    index: Option<u32>,
    name: Option<String>,
    #[serde(rename = "NetConnectionStatus")]
    net_connection_status: Option<u16>,
    #[serde(rename = "PhysicalAdapter")]
    physical_adapter: Option<bool>,
    speed: Option<u64>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Win32NetworkAdapterConfiguration {
    index: Option<u32>,
    #[serde(rename = "IPAddress")]
    ip_address: Option<Vec<String>>,
    #[serde(rename = "DefaultIPGateway")]
    default_ip_gateway: Option<Vec<String>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Win32PnPEntityBluetooth {
    name: Option<String>,
    manufacturer: Option<String>,
}

/// Get network adapter information using direct WMI (no PowerShell).
pub fn get_network_info() -> Result<NetworkInfo> {
    get_network_info_profiled().map(|(info, _)| info)
}

/// Get network info with per-phase timings.
///
/// Sub-sections:
///   adapter_list   — single WMI query for both Win32_NetworkAdapter and
///                    Win32_NetworkAdapterConfiguration
///   ip_queries     — extracting IPv4/IPv6 from the already-loaded data
///                    (effectively free, kept for reporting compatibility)
///   gateway_pings  — ICMP ping per adapter that has an IPv4 gateway
///   bluetooth      — single WMI query against Win32_PnPEntity
pub fn get_network_info_profiled() -> Result<(NetworkInfo, Vec<(String, Duration)>)> {
    let mut subs: Vec<(String, Duration)> = Vec::with_capacity(4);
    let mut ip_query_total = Duration::ZERO;
    let mut gateway_ping_total = Duration::ZERO;

    // Phase 1: open WMI connection + query adapters and configs together.
    //
    // raw_query with explicit SQL is required here — the wmi crate's class-name
    // inference returns WBEM_E_INVALID_CLASS for `Win32_NetworkAdapter` and
    // `Win32_NetworkAdapterConfiguration` even though the same crate happily
    // uses `.query()` for `Win32_BaseBoard`/`Win32_Processor`. Explicit SQL
    // sidesteps the inference and is also more selective (named columns only).
    let t = Instant::now();
    let wmi =
        WMIConnection::new().map_err(|e| MscError::other(format!("WMI connect failed: {}", e)))?;

    let adapters: Vec<Win32NetworkAdapter> = wmi
        .raw_query(
            "SELECT Index, Name, NetConnectionStatus, PhysicalAdapter, Speed \
             FROM Win32_NetworkAdapter",
        )
        .unwrap_or_default();

    let configs: Vec<Win32NetworkAdapterConfiguration> = wmi
        .raw_query(
            "SELECT Index, IPAddress, DefaultIPGateway FROM Win32_NetworkAdapterConfiguration",
        )
        .unwrap_or_default();

    subs.push(("network.adapter_list".to_string(), t.elapsed()));

    // Build Index → config lookup so the per-adapter loop is O(1).
    let mut config_by_index: HashMap<u32, &Win32NetworkAdapterConfiguration> = HashMap::new();
    for cfg in &configs {
        if let Some(idx) = cfg.index {
            config_by_index.insert(idx, cfg);
        }
    }

    // SSID of the connected WLAN interface (single native query, reused for any
    // Wi-Fi adapter below — laptops virtually always have exactly one).
    let connected_ssid = super::wifi::get_connected_ssid();

    let mut wifi_adapters = Vec::new();
    let mut ethernet_adapters = Vec::new();

    for adapter in &adapters {
        // Filter: only physical adapters that are connected (Up).
        if adapter.physical_adapter != Some(true) {
            continue;
        }
        if adapter.net_connection_status != Some(2) {
            // 2 = Connected per Win32_NetworkAdapter docs
            continue;
        }

        let description = adapter.name.clone().unwrap_or_default();
        let desc_lower = description.to_lowercase();

        let is_wifi = desc_lower.contains("wi-fi")
            || desc_lower.contains("wifi")
            || desc_lower.contains("wireless")
            || desc_lower.contains("802.11");

        let is_ethernet = !is_wifi
            && (desc_lower.contains("ethernet")
                || desc_lower.contains("gigabit")
                || desc_lower.contains("realtek") && desc_lower.contains("pcie")
                || desc_lower.contains("intel") && desc_lower.contains("connection"));

        if !is_wifi && !is_ethernet {
            continue;
        }

        // Filter out 0 and the i64::MAX sentinel that some virtual/disconnected
        // adapters report (Win32_NetworkAdapter.Speed = 9223372036854775807).
        let current_speed_mbps = adapter
            .speed
            .filter(|s| *s > 0 && *s < 100_000_000_000_000)
            .map(|s| (s / 1_000_000) as u32);

        // Phase 2: IPs from the already-loaded config (no extra WMI call).
        let t = Instant::now();
        let cfg = adapter.index.and_then(|i| config_by_index.get(&i).copied());
        let (ipv4, ipv6, gateway_ipv4) = match cfg {
            Some(c) => extract_ips_and_gateway(c),
            None => (None, None, None),
        };
        ip_query_total += t.elapsed();

        // Phase 3: ping (only when there is a gateway).
        let gateway_latency = if let Some(ref gw) = gateway_ipv4 {
            let t = Instant::now();
            let lat = ping_ipv4(gw);
            gateway_ping_total += t.elapsed();
            lat
        } else {
            None
        };

        if is_wifi {
            wifi_adapters.push(WifiAdapter {
                name: description.clone(),
                ssid: connected_ssid.clone(),
                wifi_standard: detect_wifi_standard(&description),
                bands: detect_wifi_bands(&detect_wifi_standard(&description)),
                max_speed_mbps: None,
                current_link_speed_mbps: current_speed_mbps,
                chipset_manufacturer: detect_chipset_manufacturer(&description),
                ipv4_address: ipv4,
                ipv6_address: ipv6,
                gateway_latency_ms: gateway_latency,
            });
        } else {
            ethernet_adapters.push(EthernetAdapter {
                name: description,
                speed_mbps: current_speed_mbps,
                mac_address: None,
                ipv4_address: ipv4,
                ipv6_address: ipv6,
                gateway_latency_ms: gateway_latency,
            });
        }
    }

    subs.push(("network.ip_queries".to_string(), ip_query_total));
    subs.push(("network.gateway_pings".to_string(), gateway_ping_total));

    // Phase 4: Bluetooth via WMI.
    let t = Instant::now();
    let bt: Vec<Win32PnPEntityBluetooth> = wmi
        .raw_query(
            "SELECT Name, Manufacturer FROM Win32_PnPEntity \
             WHERE PNPClass = 'Bluetooth' AND Status = 'OK'",
        )
        .unwrap_or_default();
    let bluetooth_adapters: Vec<BluetoothAdapter> = bt
        .into_iter()
        .filter_map(|d| {
            let name = d.name?;
            if name.is_empty() {
                return None;
            }
            let version = detect_bluetooth_version(&name);
            Some(BluetoothAdapter {
                name,
                version,
                is_enabled: true,
                manufacturer: d.manufacturer.filter(|s| !s.is_empty()),
            })
        })
        .collect();
    subs.push(("network.bluetooth".to_string(), t.elapsed()));

    Ok((
        NetworkInfo {
            wifi_adapters,
            ethernet_adapters,
            bluetooth_adapters,
            // Both populated by the collector via dedicated parallel threads so
            // the external lookups stay off this section's critical path.
            public_ip: None,
            internet_latency_ms: None,
        },
        subs,
    ))
}

/// Extract first IPv4 + first non-link-local IPv6 + first IPv4 default gateway.
fn extract_ips_and_gateway(
    cfg: &Win32NetworkAdapterConfiguration,
) -> (Option<String>, Option<String>, Option<String>) {
    let ipv4 = cfg
        .ip_address
        .as_ref()
        .and_then(|addrs| addrs.iter().find(|a| is_ipv4(a)).cloned());

    let ipv6 = cfg.ip_address.as_ref().and_then(|addrs| {
        addrs
            .iter()
            .find(|a| is_ipv6(a) && !a.to_lowercase().starts_with("fe80"))
            .cloned()
    });

    let gateway_ipv4 = cfg
        .default_ip_gateway
        .as_ref()
        .and_then(|gws| gws.iter().find(|g| is_ipv4(g)).cloned());

    (ipv4, ipv6, gateway_ipv4)
}

fn is_ipv4(addr: &str) -> bool {
    addr.contains('.') && !addr.contains(':')
}

fn is_ipv6(addr: &str) -> bool {
    addr.contains(':')
}

/// Ping an IPv4 address via the system `ping` command. Returns latency in ms.
fn ping_ipv4(ip: &str) -> Option<u32> {
    use std::process::Command;

    if ip.is_empty() || ip == "0.0.0.0" {
        return None;
    }

    let ping_output = Command::new("ping")
        .args(["-n", "1", "-w", "1000", ip])
        .output()
        .ok()?;

    if !ping_output.status.success() {
        return None;
    }

    let ping_str = String::from_utf8_lossy(&ping_output.stdout);
    parse_ping_latency_ms(&ping_str)
}

/// Parse the latency (ms) from ONE `ping` reply line.
///
/// Handles both separators Windows emits after `time`/`tiempo`:
///   * `time=11ms` -> `Some(11)`
///   * `time<1ms`  -> `Some(0)`  (sub-millisecond; Windows prints `<`, not `=`)
///
/// Non-reply lines (the `...times in milli-seconds` header, the
/// `Minimum = 0ms` footer, `Request timed out.`) carry no `time`/`tiempo` token
/// next to an `ms`, so they yield `None` and are skipped by the callers.
fn parse_ping_line_ms(line: &str) -> Option<u32> {
    let idx = line.find("time").or_else(|| line.find("tiempo"))?;
    let rest = &line[idx..];
    let ms_pos = rest.find("ms")?;

    let token = &rest[..ms_pos]; // e.g. "time<1" or "time=11"
    if token.contains('<') {
        return Some(0); // "<1ms" — honest sub-millisecond
    }
    let digits: String = token.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok()
}

/// Latency (ms) from a single-shot `ping`: the first parseable reply.
///
/// We deliberately do NOT fall back to wall-clock timing: the old code returned
/// the `ping.exe` *process spawn* time (~15 ms) whenever parsing failed, which
/// is exactly the `time<1ms` case — so a sub-millisecond wired gateway was
/// reported as ~15 ms and looked SLOWER than a genuine ~11 ms Wi-Fi link. Spawn
/// time is not network latency; when we can't read a real RTT we say so (`None`).
fn parse_ping_latency_ms(output: &str) -> Option<u32> {
    output.lines().find_map(parse_ping_line_ms)
}

/// Lowest successful RTT across several independent single-shot pings.
///
/// Taking the min strips single-sample jitter and the cold-start spike of the
/// first packet after a Wi-Fi radio wakes, giving a stable "best case" RTT.
/// `None` only when every ping failed/timed out — never a fabricated value.
fn min_successful_latency(results: impl IntoIterator<Item = Option<u32>>) -> Option<u32> {
    results.into_iter().flatten().min()
}

/// One single-shot `ping` to Cloudflare's `1.1.1.1`, returning its RTT in ms.
fn ping_internet_once() -> Option<u32> {
    use std::process::Command;

    let out = Command::new("ping")
        .args(["-n", "1", "-w", "1000", "1.1.1.1"])
        .output()
        .ok()?;

    // Windows `ping` exits 0 only when the single echo replied.
    if !out.status.success() {
        return None;
    }

    parse_ping_latency_ms(&String::from_utf8_lossy(&out.stdout))
}

/// Best-effort internet (WAN) latency: the min of 3 CONCURRENT pings to
/// Cloudflare's `1.1.1.1` anycast resolver.
///
/// Concurrency is the whole point. Windows `ping` spaces successive packets
/// ~1 second apart, so a single `ping -n 3` costs ~2s and — running on the
/// collector's blocking thread scope — dominated the entire `sys info`
/// wall-clock (a 738ms command ballooned to 2.10s). Three parallel `-n 1`
/// pings cost ~one round-trip while still stripping first-packet jitter via the
/// min. `1.1.1.1` needs no DNS (works even when resolution is broken) and is
/// reachable from virtually everywhere. Returns `None` when offline / every
/// echo times out — never a fabricated value.
pub fn measure_internet_latency() -> Option<u32> {
    let handles: Vec<_> = (0..3)
        .map(|_| std::thread::spawn(ping_internet_once))
        .collect();

    min_successful_latency(handles.into_iter().map(|h| h.join().ok().flatten()))
}

/// Parse link speed string (e.g., "2.4 Gbps" -> 2400 Mbps) — kept for any callers
/// that still receive a string speed. Currently unused after WMI migration.
#[allow(dead_code)]
fn parse_link_speed(link_speed: Option<&str>) -> Option<u32> {
    link_speed.and_then(|speed_str| {
        let speed = speed_str.trim().to_lowercase();
        if speed.contains("gbps") {
            let gbps: f32 = speed.replace("gbps", "").trim().parse().ok()?;
            Some((gbps * 1000.0) as u32)
        } else if speed.contains("mbps") {
            let mbps: f32 = speed.replace("mbps", "").trim().parse().ok()?;
            Some(mbps as u32)
        } else {
            None
        }
    })
}

fn detect_chipset_manufacturer(description: &str) -> Option<String> {
    let desc_lower = description.to_lowercase();

    let manufacturers = [
        ("realtek", "Realtek"),
        ("intel", "Intel"),
        ("qualcomm", "Qualcomm"),
        ("atheros", "Qualcomm Atheros"),
        ("broadcom", "Broadcom"),
        ("mediatek", "MediaTek"),
        ("marvell", "Marvell"),
        ("ralink", "Ralink"),
    ];

    for (pattern, name) in manufacturers {
        if desc_lower.contains(pattern) {
            return Some(name.to_string());
        }
    }

    None
}

fn detect_bluetooth_version(name: &str) -> BluetoothVersion {
    let name_lower = name.to_lowercase();

    if name_lower.contains("5.4") {
        BluetoothVersion::V5_4
    } else if name_lower.contains("5.3") {
        BluetoothVersion::V5_3
    } else if name_lower.contains("5.2") {
        BluetoothVersion::V5_2
    } else if name_lower.contains("5.1") {
        BluetoothVersion::V5_1
    } else if name_lower.contains("5.0") || name_lower.contains("5") {
        BluetoothVersion::V5_0
    } else if name_lower.contains("4.2") {
        BluetoothVersion::V4_2
    } else if name_lower.contains("4.1") {
        BluetoothVersion::V4_1
    } else if name_lower.contains("4.0") || name_lower.contains("4") {
        BluetoothVersion::V4_0
    } else {
        BluetoothVersion::Unknown
    }
}

fn detect_wifi_standard(name: &str) -> WifiStandard {
    let name_lower = name.to_lowercase();

    if name_lower.contains("wi-fi 7")
        || name_lower.contains("wifi 7")
        || name_lower.contains("be200")
    {
        WifiStandard::WiFi7
    } else if name_lower.contains("6e")
        || name_lower.contains("ax211")
        || name_lower.contains("ax210")
    {
        WifiStandard::WiFi6E
    } else if name_lower.contains("wi-fi 6")
        || name_lower.contains("wifi 6")
        || name_lower.contains("ax")
        || name_lower.contains("ax200")
        || name_lower.contains("ax201")
    {
        WifiStandard::WiFi6
    } else if name_lower.contains("ac") {
        WifiStandard::WiFi5
    } else if name_lower.contains("n") {
        WifiStandard::WiFi4
    } else {
        WifiStandard::Unknown
    }
}

fn detect_wifi_bands(standard: &WifiStandard) -> Vec<WifiBand> {
    match standard {
        WifiStandard::WiFi7 | WifiStandard::WiFi6E => {
            vec![WifiBand::Band2_4GHz, WifiBand::Band5GHz, WifiBand::Band6GHz]
        }
        WifiStandard::WiFi6 | WifiStandard::WiFi5 => {
            vec![WifiBand::Band2_4GHz, WifiBand::Band5GHz]
        }
        WifiStandard::WiFi4 => {
            vec![WifiBand::Band2_4GHz, WifiBand::Band5GHz]
        }
        WifiStandard::Unknown => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::parse_ping_latency_ms;

    #[test]
    fn parses_explicit_millis() {
        let out = "Reply from 192.168.1.1: bytes=32 time=11ms TTL=64";
        assert_eq!(parse_ping_latency_ms(out), Some(11));
    }

    #[test]
    fn sub_millisecond_is_zero_not_spawn_time() {
        // The bug case: Windows prints `time<1ms` for a fast wired gateway.
        let out = "Reply from 192.168.1.1: bytes=32 time<1ms TTL=64";
        assert_eq!(parse_ping_latency_ms(out), Some(0));
    }

    #[test]
    fn parses_spanish_locale() {
        let out = "Respuesta desde 192.168.1.1: bytes=32 tiempo=7ms TTL=64";
        assert_eq!(parse_ping_latency_ms(out), Some(7));
        let out_sub = "Respuesta desde 192.168.1.1: bytes=32 tiempo<1ms TTL=64";
        assert_eq!(parse_ping_latency_ms(out_sub), Some(0));
    }

    #[test]
    fn ignores_stats_footer_and_picks_reply_line() {
        // Full Windows output: the "Minimum = 0ms, ..." footer also contains
        // "ms" but no time token; the reply line must be the one that wins.
        let out = "\nPinging 192.168.1.1 with 32 bytes of data:\n\
                   Reply from 192.168.1.1: bytes=32 time=3ms TTL=64\n\n\
                   Ping statistics for 192.168.1.1:\n\
                   Approximate round trip times in milli-seconds:\n\
                   Minimum = 0ms, Maximum = 0ms, Average = 0ms";
        assert_eq!(parse_ping_latency_ms(out), Some(3));
    }

    #[test]
    fn no_reply_yields_none() {
        let out = "Request timed out.";
        assert_eq!(parse_ping_latency_ms(out), None);
    }

    #[test]
    fn min_picks_lowest_across_concurrent_pings() {
        // 3 concurrent single-shot pings with jitter — min strips the spike.
        let results = [Some(46), Some(44), Some(45)];
        assert_eq!(super::min_successful_latency(results), Some(44));
    }

    #[test]
    fn min_ignores_failed_pings() {
        // A timed-out ping yields None; the live replies still win.
        let results = [None, Some(50), None];
        assert_eq!(super::min_successful_latency(results), Some(50));
    }

    #[test]
    fn min_of_all_failures_is_none() {
        let results = [None, None, None];
        assert_eq!(super::min_successful_latency(results), None);
    }
}
