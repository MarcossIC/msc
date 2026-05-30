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
    #[serde(rename = "NetConnectionID")]
    net_connection_id: Option<String>,
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
            "SELECT Index, Name, NetConnectionID, NetConnectionStatus, PhysicalAdapter, Speed \
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

    let start = Instant::now();
    let ping_output = Command::new("ping")
        .args(["-n", "1", "-w", "1000", ip])
        .output()
        .ok()?;
    let elapsed = start.elapsed();

    if !ping_output.status.success() {
        return None;
    }

    let ping_str = String::from_utf8_lossy(&ping_output.stdout);
    if let Some(time_line) = ping_str
        .lines()
        .find(|l| l.contains("time=") || l.contains("tiempo="))
    {
        if let Some(pos) = time_line
            .find("time=")
            .or_else(|| time_line.find("tiempo="))
        {
            let time_str = &time_line[pos..];
            if let Some(ms_pos) = time_str.find("ms") {
                let val = time_str[5..ms_pos].trim();
                if let Ok(latency) = val.replace("<", "").parse::<u32>() {
                    return Some(latency);
                }
            }
        }
    }

    Some(elapsed.as_millis() as u32)
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
