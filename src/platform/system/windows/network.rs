use crate::core::system_info::types::{
    BluetoothAdapter, BluetoothVersion, EthernetAdapter, NetworkInfo, WifiAdapter, WifiBand,
    WifiStandard,
};
use crate::error::{MscError, Result};

/// Get network adapter information using PowerShell
pub fn get_network_info() -> Result<NetworkInfo> {
    use std::process::Command;

    // Get all network adapters with extended information
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-NetAdapter | Where-Object {$_.Status -eq 'Up'} | Select-Object Name, InterfaceDescription, LinkSpeed | ConvertTo-Json"
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to run PowerShell: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Parse JSON output
    let json_value: serde_json::Value = serde_json::from_str(&output_str)
        .map_err(|e| MscError::other(format!("Failed to parse JSON: {}", e)))?;

    // Handle both single object and array
    let adapter_array = if json_value.is_array() {
        json_value.as_array().unwrap().clone()
    } else {
        vec![json_value.clone()]
    };

    let mut wifi_adapters = Vec::new();
    let mut ethernet_adapters = Vec::new();

    for adapter_json in adapter_array.iter() {
        let name = adapter_json["Name"].as_str().unwrap_or("").to_string();
        let description = adapter_json["InterfaceDescription"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let link_speed = adapter_json["LinkSpeed"].as_str();

        if name.is_empty() {
            continue;
        }

        // Parse link speed (e.g., "2.4 Gbps" -> 2400 Mbps)
        let current_speed_mbps = parse_link_speed(link_speed);

        // Detect WiFi adapters
        if description.to_lowercase().contains("wi-fi")
            || description.to_lowercase().contains("wifi")
            || description.to_lowercase().contains("wireless")
            || description.to_lowercase().contains("802.11")
        {
            let wifi_standard = detect_wifi_standard(&description);
            let bands = detect_wifi_bands(&wifi_standard);
            let chipset_manufacturer = detect_chipset_manufacturer(&description);

            // Get IP addresses and gateway latency
            let (ipv4, ipv6, gateway_latency) = get_adapter_network_details(&name);

            wifi_adapters.push(WifiAdapter {
                name: description.clone(),
                wifi_standard,
                bands,
                max_speed_mbps: None,
                current_link_speed_mbps: current_speed_mbps,
                chipset_manufacturer,
                ipv4_address: ipv4,
                ipv6_address: ipv6,
                gateway_latency_ms: gateway_latency,
            });
        }
        // Detect Ethernet adapters
        else if description.to_lowercase().contains("ethernet")
            || description.to_lowercase().contains("gigabit")
            || description.to_lowercase().contains("realtek")
                && description.to_lowercase().contains("pcie")
            || description.to_lowercase().contains("intel")
                && description.to_lowercase().contains("connection")
        {
            // Get IP addresses and gateway latency
            let (ipv4, ipv6, gateway_latency) = get_adapter_network_details(&name);

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

    // Get Bluetooth adapters
    let bluetooth_adapters = get_bluetooth_adapters().unwrap_or_default();

    Ok(NetworkInfo {
        wifi_adapters,
        ethernet_adapters,
        bluetooth_adapters,
    })
}

/// Parse link speed string (e.g., "2.4 Gbps" -> 2400 Mbps)
fn parse_link_speed(link_speed: Option<&str>) -> Option<u32> {
    link_speed.and_then(|speed_str| {
        // Remove spaces and convert to lowercase
        let speed = speed_str.trim().to_lowercase();

        // Check for Gbps
        if speed.contains("gbps") {
            let gbps: f32 = speed.replace("gbps", "").trim().parse().ok()?;
            Some((gbps * 1000.0) as u32)
        }
        // Check for Mbps
        else if speed.contains("mbps") {
            let mbps: f32 = speed.replace("mbps", "").trim().parse().ok()?;
            Some(mbps as u32)
        } else {
            None
        }
    })
}

/// Detect chipset manufacturer from adapter description
fn detect_chipset_manufacturer(description: &str) -> Option<String> {
    let desc_lower = description.to_lowercase();

    let manufacturers = vec![
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

/// Get IP addresses and gateway latency for a network adapter
fn get_adapter_network_details(
    adapter_name: &str,
) -> (Option<String>, Option<String>, Option<u32>) {
    use std::process::Command;

    // Get IP configuration for this adapter
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "$adapter = Get-NetAdapter -Name '{}' -ErrorAction SilentlyContinue; \
                 $ipv4 = Get-NetIPAddress -InterfaceAlias '{}' -AddressFamily IPv4 -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty IPAddress; \
                 $ipv6 = Get-NetIPAddress -InterfaceAlias '{}' -AddressFamily IPv6 -ErrorAction SilentlyContinue | Where-Object {{$_.PrefixOrigin -ne 'WellKnown'}} | Select-Object -First 1 -ExpandProperty IPAddress; \
                 @{{IPv4 = $ipv4; IPv6 = $ipv6}} | ConvertTo-Json",
                adapter_name, adapter_name, adapter_name
            )
        ])
        .output()
        .ok();

    let mut ipv4 = None;
    let mut ipv6 = None;

    if let Some(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
            ipv4 = json["IPv4"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            ipv6 = json["IPv6"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
        }
    }

    // Get gateway and measure latency
    let gateway_latency = if ipv4.is_some() {
        get_gateway_latency(adapter_name)
    } else {
        None
    };

    (ipv4, ipv6, gateway_latency)
}

/// Measure latency to default gateway
fn get_gateway_latency(adapter_name: &str) -> Option<u32> {
    use std::process::Command;
    use std::time::Instant;

    // Get default gateway for this adapter
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Get-NetRoute -InterfaceAlias '{}' -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty NextHop",
                adapter_name
            )
        ])
        .output()
        .ok()?;

    let gateway = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if gateway.is_empty() || gateway == "0.0.0.0" {
        return None;
    }

    // Ping the gateway once to measure latency
    let start = Instant::now();
    let ping_output = Command::new("ping")
        .args(["-n", "1", "-w", "1000", &gateway])
        .output()
        .ok()?;
    let elapsed = start.elapsed();

    if !ping_output.status.success() {
        return None;
    }

    // Parse ping output for actual latency
    let ping_str = String::from_utf8_lossy(&ping_output.stdout);
    if let Some(time_line) = ping_str
        .lines()
        .find(|line| line.contains("time=") || line.contains("tiempo="))
    {
        // Extract time in ms
        if let Some(time_start) = time_line
            .find("time=")
            .or_else(|| time_line.find("tiempo="))
        {
            let time_str = &time_line[time_start..];
            if let Some(ms_pos) = time_str.find("ms") {
                let time_value = &time_str[5..ms_pos].trim();
                if let Ok(latency) = time_value.replace("<", "").parse::<u32>() {
                    return Some(latency);
                }
            }
        }
    }

    // Fallback to elapsed time
    Some(elapsed.as_millis() as u32)
}

/// Get Bluetooth adapters
fn get_bluetooth_adapters() -> Result<Vec<BluetoothAdapter>> {
    use std::process::Command;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-PnpDevice -Class Bluetooth | Where-Object {$_.Status -eq 'OK'} | Select-Object FriendlyName, Manufacturer | ConvertTo-Json"
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to get Bluetooth info: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    if output_str.trim().is_empty() || output_str.trim() == "[]" {
        return Ok(Vec::new());
    }

    let json_value: serde_json::Value = serde_json::from_str(&output_str)
        .map_err(|e| MscError::other(format!("Failed to parse Bluetooth JSON: {}", e)))?;

    let mut bluetooth_adapters = Vec::new();

    let bt_array = if json_value.is_array() {
        json_value.as_array().unwrap()
    } else {
        &vec![json_value.clone()]
    };

    for bt_json in bt_array {
        let name = bt_json["FriendlyName"]
            .as_str()
            .unwrap_or("Unknown Bluetooth Adapter")
            .to_string();
        let manufacturer = bt_json["Manufacturer"].as_str().map(|s| s.to_string());

        // Detect Bluetooth version from name
        let version = detect_bluetooth_version(&name);

        bluetooth_adapters.push(BluetoothAdapter {
            name,
            version,
            is_enabled: true, // If Status is OK, it's enabled
            manufacturer,
        });
    }

    Ok(bluetooth_adapters)
}

/// Detect Bluetooth version from adapter name
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
