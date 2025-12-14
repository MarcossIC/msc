use crate::core::system_info::types::*;
use crate::error::{MscError, Result};
use serde::Deserialize;
#[cfg(windows)]
use wmi::WMIConnection;

// WMI Structs
// #[derive(Deserialize, Debug)]
// #[serde(rename_all = "PascalCase")]
// struct Win32PhysicalMemory {
//     capacity: Option<String>,
//     speed: Option<u32>,
//     manufacturer: Option<String>,
//     part_number: Option<String>,
//     device_locator: Option<String>,
//     smbios_memory_type: Option<u16>,
// }

// #[derive(Deserialize, Debug)]
// #[serde(rename_all = "PascalCase")]
// struct Win32PhysicalMemoryArray {
//     memory_devices: Option<u32>,
//     max_capacity: Option<u64>,
// }

// #[derive(Deserialize, Debug)]
// #[serde(rename_all = "PascalCase")]
// struct Win32VideoController {
//     name: Option<String>,
//     adapter_ram: Option<String>,
// }

// #[derive(Deserialize, Debug)]
// #[serde(rename_all = "PascalCase")]
// struct Win32NetworkAdapter {
//     name: Option<String>,
//     adapter_type: Option<String>,
//     speed: Option<String>,
// }

// #[derive(Deserialize, Debug)]
// #[serde(rename_all = "PascalCase")]
// struct Win32Battery {
//     battery_status: Option<u16>,
//     estimated_charge_remaining: Option<u16>,
//     estimated_run_time: Option<u32>,
//     design_capacity: Option<u32>,
//     full_charge_capacity: Option<u32>,
//     chemistry: Option<u16>,
//     design_voltage: Option<u64>,
//     discharge_rate: Option<i32>,
//     manufacturer: Option<String>,
//     serial_number: Option<String>,
//     manufacture_date: Option<String>,
// }

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Win32Processor {
    max_clock_speed: Option<u32>,
    turbo_mode_enabled: Option<bool>,
    l2_cache_size: Option<u32>, // KB
    l3_cache_size: Option<u32>, // KB
    _number_of_cores: Option<u32>,
    _number_of_logical_processors: Option<u32>,
}

// #[derive(Deserialize, Debug)]
// #[serde(rename_all = "PascalCase")]
// struct Win32DiskDrive {
//     model: Option<String>,
//     media_type: Option<String>,
//     interface_type: Option<String>,
//     caption: Option<String>,
// }
#[cfg(windows)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
struct Win32DiskDrive {
    model: Option<String>,
    serial_number: Option<String>,
    interface_type: Option<String>, // IDE, SCSI, USB
    media_type: Option<String>,     // Fixed hard disk media
    status: Option<String>,         // OK, Pred Fail, etc.
    size: Option<u64>,
    device_id: String,
}

/// Get detailed memory information using PowerShell
#[derive(Default)]
pub struct MemoryDetails {
    pub ddr_type: Option<DdrType>,
    pub speed_mhz: Option<u32>,
    pub modules: Vec<MemoryModule>,
    pub total_slots: Option<u32>,
    pub used_slots: Option<u32>,
    pub max_capacity_bytes: Option<u64>,
}

/// Get detailed memory information using PowerShell
pub fn get_memory_details() -> Result<MemoryDetails> {
    use std::process::Command;

    // Query for physical memory modules
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance -ClassName Win32_PhysicalMemory | Select-Object Capacity, Speed, Manufacturer, PartNumber, DeviceLocator, SMBIOSMemoryType | ConvertTo-Json"
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to run PowerShell: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    let json_value: serde_json::Value = serde_json::from_str(&output_str)
        .map_err(|e| MscError::other(format!("Failed to parse JSON: {}", e)))?;

    // Handle both single object and array
    let mem_array = if json_value.is_array() {
        json_value.as_array().unwrap().clone()
    } else {
        vec![json_value.clone()]
    };

    let mut modules = Vec::new();
    let mut ddr_type = None;
    let mut speed_mhz = None;

    for mem_json in mem_array.iter() {
        let capacity = mem_json["Capacity"].as_u64().unwrap_or(0);
        let speed = mem_json["Speed"].as_u64().map(|s| s as u32);
        let smbios_type = mem_json["SMBIOSMemoryType"].as_u64().map(|t| t as u16);
        let manufacturer = mem_json["Manufacturer"].as_str().map(|s| s.to_string());
        let part_number = mem_json["PartNumber"]
            .as_str()
            .map(|s| s.trim().to_string());
        let device_locator = mem_json["DeviceLocator"].as_str().map(|s| s.to_string());

        // Determine DDR type from SMBIOSMemoryType
        if ddr_type.is_none() {
            ddr_type = Some(match smbios_type {
                Some(18) => DdrType::DDR,
                Some(19) => DdrType::DDR2,
                Some(24) => DdrType::DDR3,
                Some(26) => DdrType::DDR4,
                Some(34) => DdrType::DDR5,
                Some(_) => DdrType::Unknown,
                None => DdrType::Unknown,
            });
        }

        if speed_mhz.is_none() && speed.is_some() {
            speed_mhz = speed;
        }

        modules.push(MemoryModule {
            capacity_bytes: capacity,
            speed_mhz: speed,
            manufacturer,
            part_number,
            slot: device_locator,
        });
    }

    // Query for total slots
    let output2 = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance -ClassName Win32_PhysicalMemoryArray | Select-Object MemoryDevices, MaxCapacity | ConvertTo-Json"
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to run PowerShell: {}", e)))?;

    let output2_str = String::from_utf8_lossy(&output2.stdout);

    let json_value2: serde_json::Value = serde_json::from_str(&output2_str)
        .map_err(|e| MscError::other(format!("Failed to parse JSON: {}", e)))?;

    let array_data = if json_value2.is_array() {
        json_value2.as_array().and_then(|arr| arr.first())
    } else {
        Some(&json_value2)
    };

    let total_slots = array_data
        .and_then(|a| a["MemoryDevices"].as_u64())
        .map(|d| d as u32);
    let max_capacity = array_data
        .and_then(|a| a["MaxCapacity"].as_u64())
        .map(|kb| kb * 1024); // Convert KB to bytes
    let used_slots = Some(modules.len() as u32);

    Ok(MemoryDetails {
        ddr_type,
        speed_mhz,
        modules,
        total_slots,
        used_slots,
        max_capacity_bytes: max_capacity,
    })
}

/// Get GPU information using PowerShell
pub fn get_gpu_info() -> Result<Vec<GpuInfo>> {
    use std::process::Command;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance -ClassName Win32_VideoController | Select-Object Name, AdapterRAM | ConvertTo-Json"
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to run PowerShell: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Parse JSON output
    let json_value: serde_json::Value = serde_json::from_str(&output_str)
        .map_err(|e| MscError::other(format!("Failed to parse JSON: {}", e)))?;

    let mut gpus = Vec::new();

    // Handle both single object and array
    let gpu_array = if json_value.is_array() {
        json_value.as_array().unwrap()
    } else {
        &vec![json_value.clone()]
    };

    let mut nvidia_gpu_index = 0u32; // Separate counter for NVIDIA GPUs (for NVML indexing)

    for gpu_json in gpu_array.iter() {
        let name = gpu_json["Name"]
            .as_str()
            .unwrap_or("Unknown GPU")
            .to_string();
        let adapter_ram = gpu_json["AdapterRAM"].as_u64();

        // Skip basic display adapters
        if name.contains("Basic Display") || name.contains("Microsoft Basic") {
            continue;
        }

        let vendor = if name.contains("NVIDIA") {
            "NVIDIA"
        } else if name.contains("AMD") || name.contains("Radeon") {
            "AMD"
        } else if name.contains("Intel") {
            "Intel"
        } else {
            "Unknown"
        }
        .to_string();

        let is_integrated = name.contains("Intel") && !name.contains("Arc")
            || (name.contains("AMD") && name.contains("Radeon") && !name.contains("RX"));

        // Get advanced metrics based on vendor
        let (
            nvidia_metrics,
            amd_metrics,
            core_clock,
            mem_clock,
            temp,
            power_draw,
            fan_speed,
            memory_type,
            vram_bytes_corrected,
        ) = if vendor == "NVIDIA" {
            // Try NVML first (official NVIDIA library with real data)
            #[cfg(feature = "nvml")]
            {
                if let Ok(nvidia) =
                    crate::platform::nvidia_nvml::get_nvidia_metrics_nvml(nvidia_gpu_index)
                {
                    // Extract real-time metrics from NvidiaGpuMetrics
                    let core_clock = nvidia.clock_graphics_mhz;
                    let mem_clock = nvidia.clock_memory_mhz;
                    let temp = nvidia.temperature_celsius;
                    let power = nvidia.power_draw_watts;
                    let fan = nvidia.fan_speed_percent;

                    // Get REAL VRAM from NVML (fixes 4GB limit from Win32_VideoController)
                    let vram = nvidia.memory_total_bytes;

                    // Try to get memory type using smart detection
                    let mem_type = crate::platform::nvidia_nvml::get_gpu_memory_type(None);

                    // Increment NVIDIA GPU counter
                    nvidia_gpu_index += 1;

                    (
                        Some(nvidia),
                        None,
                        core_clock,
                        mem_clock,
                        temp,
                        power,
                        fan,
                        mem_type,
                        vram,
                    )
                } else {
                    // Fallback to nvidia-smi (use same nvidia_gpu_index for query)
                    let (core, mem, temp, power, fan) =
                        get_nvidia_realtime_metrics_by_index(nvidia_gpu_index)
                            .unwrap_or((None, None, None, None, None));
                    let vram_corrected =
                        get_nvidia_vram_from_smi_by_index(nvidia_gpu_index).or(adapter_ram);

                    // Increment NVIDIA GPU counter
                    nvidia_gpu_index += 1;

                    (
                        None,
                        None,
                        core,
                        mem,
                        temp,
                        power,
                        fan,
                        None,
                        vram_corrected,
                    )
                }
            }
            #[cfg(not(feature = "nvml"))]
            {
                // NVML not available, use nvidia-smi
                let (core, mem, temp, power, fan) =
                    get_nvidia_realtime_metrics_by_index(nvidia_gpu_index)
                        .unwrap_or((None, None, None, None, None));
                let vram_corrected =
                    get_nvidia_vram_from_smi_by_index(nvidia_gpu_index).or(adapter_ram);

                // Increment NVIDIA GPU counter
                nvidia_gpu_index += 1;

                (
                    None,
                    None,
                    core,
                    mem,
                    temp,
                    power,
                    fan,
                    None,
                    vram_corrected,
                )
            }
        } else if vendor == "AMD" {
            let amd = get_amd_metrics(&name);
            let (core, mem, temp, power, fan) = get_gpu_realtime_metrics(&name, &vendor);
            let mem_type = get_amd_gpu_memory_type(&name);
            (
                None,
                Some(amd),
                core,
                mem,
                temp,
                power,
                fan,
                mem_type,
                adapter_ram,
            )
        } else {
            (None, None, None, None, None, None, None, None, adapter_ram)
        };

        gpus.push(GpuInfo {
            name,
            vendor,
            vram_bytes: vram_bytes_corrected,
            memory_type,
            is_integrated,
            driver_version: None, // Could be obtained from registry
            core_clock_mhz: core_clock,
            memory_clock_mhz: mem_clock,
            temperature_celsius: temp,
            power_draw_watts: power_draw,
            fan_speed_percent: fan_speed,
            nvidia_metrics,
            amd_metrics,
        });
    }

    Ok(gpus)
}

/// Fallback memory type detection for AMD GPUs
///
/// For AMD GPUs, we use a simplified detection since we don't have
/// direct hardware access like NVML for NVIDIA
fn get_amd_gpu_memory_type(model: &str) -> Option<String> {
    // RX 7000 and 6000 Series use GDDR6
    if model.contains("RX 7") || model.contains("RX 6") {
        return Some("GDDR6".to_string());
    }

    None
}

/// Get motherboard information using PowerShell
#[cfg(windows)]
pub fn get_motherboard_info() -> Result<MotherboardInfo> {
    use std::process::Command;

    // Query baseboard and BIOS info using PowerShell (more reliable than direct WMI)
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$baseboard = Get-CimInstance -ClassName Win32_BaseBoard -ErrorAction SilentlyContinue | Select-Object -First 1; \
             $bios = Get-CimInstance -ClassName Win32_BIOS -ErrorAction SilentlyContinue | Select-Object -First 1; \
             @{ \
                 Manufacturer = $baseboard.Manufacturer; \
                 Product = $baseboard.Product; \
                 Version = $baseboard.Version; \
                 BiosVendor = $bios.Manufacturer; \
                 BiosVersion = $bios.SMBIOSBIOSVersion \
             } | ConvertTo-Json"
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to run PowerShell: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Parse motherboard info
    let (manufacturer, product, version, bios_vendor, bios_version) =
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
            let manufacturer = json["Manufacturer"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            let product = json["Product"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            let version = json["Version"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            let bios_vendor = json["BiosVendor"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            let bios_version = json["BiosVersion"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());

            (manufacturer, product, version, bios_vendor, bios_version)
        } else {
            (None, None, None, None, None)
        };

    // Get chipset information
    let chipset = detect_chipset();

    // Get TPM version
    let tpm_version = detect_tpm_version();

    // Get DIMM slots count (from memory module query)
    let dimm_slots = get_dimm_slot_count();

    Ok(MotherboardInfo {
        manufacturer,
        product,
        version,
        bios_vendor,
        bios_version,
        chipset,
        tpm_version,
        dimm_slots,
        pcie_slots: None, // Would require Win32_SystemSlot query
        m2_slots_total: None,
        m2_slots_used: None,
    })
}

/// Detect motherboard chipset using PowerShell and WMI
///
/// This function queries PnP devices and USB controllers to identify the chipset.
fn detect_chipset() -> Option<String> {
    use std::process::Command;

    // Try to get chipset from Win32_Bus or PnP devices
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance -ClassName Win32_PnPEntity | Where-Object { $_.Name -like '*Chipset*' -or $_.Name -like '*SMBus*' -or $_.Name -like '*LPC Controller*' } | Select-Object -First 1 -ExpandProperty Name"
        ])
        .output()
        .ok()?;

    let chipset_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !chipset_str.is_empty() {
        // Extract chipset name from device string
        // Example: "Intel(R) 600 Series Chipset Family LPC Controller (Z690)"
        return Some(extract_chipset_name(&chipset_str));
    }

    None
}

/// Extract chipset name from PnP device string
fn extract_chipset_name(device_name: &str) -> String {
    // Common patterns:
    // "Intel(R) 600 Series Chipset Family ..." -> "Intel 600 Series"
    // "AMD X670 Chipset" -> "AMD X670"

    if device_name.contains("Intel") {
        if let Some(start) = device_name.find("Intel") {
            let rest = &device_name[start..];
            if let Some(end) = rest.find("Chipset") {
                let mut chipset = rest[..end + 7].to_string(); // Include "Chipset"
                chipset = chipset
                    .replace("(R)", "")
                    .trim()
                    .to_string();
                return chipset;
            }
        }
    } else if device_name.contains("AMD") {
        if let Some(start) = device_name.find("AMD") {
            let rest = &device_name[start..];
            if let Some(end) = rest.find("Chipset") {
                return rest[..end + 7].trim().to_string();
            }
        }
    }

    // Fallback: return cleaned device name
    device_name.replace("(R)", "").trim().to_string()
}

/// Detect TPM version using PowerShell
///
/// This function queries Win32_Tpm WMI class to get the TPM specification version.
fn detect_tpm_version() -> Option<crate::core::system_info::types::TpmVersion> {
    use crate::core::system_info::types::TpmVersion;
    use std::process::Command;

    // Query TPM version from WMI
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-WmiObject -Namespace root\\CIMV2\\Security\\MicrosoftTpm -Class Win32_Tpm -ErrorAction SilentlyContinue | Select-Object -ExpandProperty SpecVersion"
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if version_str.is_empty() {
        return None;
    }

    // Parse version string (e.g., "1.2, 2", "2.0")
    if version_str.starts_with("2.") || version_str.contains(", 2") || version_str == "2" {
        Some(TpmVersion::V2_0)
    } else if version_str.starts_with("1.2") || version_str.contains("1.2") {
        Some(TpmVersion::V1_2)
    } else {
        Some(TpmVersion::Unknown)
    }
}

/// Get the number of DIMM slots from Win32_PhysicalMemoryArray
fn get_dimm_slot_count() -> Option<u32> {
    use std::process::Command;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance -ClassName Win32_PhysicalMemoryArray | Select-Object -First 1 -ExpandProperty MemoryDevices"
        ])
        .output()
        .ok()?;

    let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    count_str.parse::<u32>().ok()
}

#[cfg(not(windows))]
pub fn get_motherboard_info() -> Result<MotherboardInfo> {
    Err(MscError::other(
        "get_motherboard_info sólo está disponible en Windows",
    ))
}

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

/// Get battery information using PowerShell
/// Get comprehensive battery information using multiple Windows APIs
///
/// This function combines data from:
/// 1. Win32_Battery - Basic status and state
/// 2. BatteryStaticData/BatteryFullChargedCapacity - Accurate capacity via WMI
/// 3. Registry - Cycle count and additional metrics
pub fn get_battery_info() -> Result<BatteryInfo> {
    use std::process::Command;

    // First check if battery exists at all
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance -ClassName Win32_Battery | ConvertTo-Json",
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to run PowerShell: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    // If no output or empty array, no battery present (desktop PC)
    if output_str.trim().is_empty() || output_str.trim() == "[]" {
        return Ok(BatteryInfo {
            is_present: false,
            state: BatteryState::Unknown,
            percentage: None,
            time_remaining_secs: None,
            time_to_full_secs: None,
            design_capacity_mwh: None,
            full_charge_capacity_mwh: None,
            health_percentage: None,
            cycle_count: None,
            technology: None,
            voltage_mv: None,
            design_voltage_mv: None,
            discharge_rate_mw: None,
            manufacturer: None,
            serial_number: None,
            manufacture_date: None,
        });
    }

    // Parse basic battery info
    let json_value: serde_json::Value = serde_json::from_str(&output_str)
        .map_err(|e| MscError::other(format!("Failed to parse JSON: {}", e)))?;

    let battery_json = if json_value.is_array() {
        json_value
            .as_array()
            .and_then(|arr| arr.first())
            .ok_or_else(|| MscError::other("Battery array is empty"))?
    } else {
        &json_value
    };

    // Get basic state from Win32_Battery
    let battery_status = battery_json["BatteryStatus"].as_u64().map(|s| s as u16);
    let state = match battery_status {
        Some(1) => BatteryState::Discharging,
        Some(2) => BatteryState::Charging,
        Some(3) => BatteryState::Full,
        Some(11) => BatteryState::NotCharging,
        _ => BatteryState::Unknown,
    };

    let percentage = battery_json["EstimatedChargeRemaining"]
        .as_u64()
        .map(|p| p as u8);
    let time_remaining_secs = battery_json["EstimatedRunTime"].as_u64().and_then(|t| {
        if t < 71582788 {
            Some((t * 60) as u32)
        } else {
            None
        }
    }); // Filter out invalid values

    // Get chemistry/technology
    let chemistry = battery_json["Chemistry"].as_u64().map(|c| c as u16);
    let technology = match chemistry {
        Some(1) => Some(BatteryTechnology::LeadAcid),
        Some(2) => Some(BatteryTechnology::NickelCadmium),
        Some(3) => Some(BatteryTechnology::NickelMetalHydride),
        Some(4) | Some(6) => Some(BatteryTechnology::LithiumIon),
        Some(7) => Some(BatteryTechnology::LithiumPolymer),
        _ => Some(BatteryTechnology::Unknown),
    };

    // Get capacity information using more reliable WMI classes
    let (design_capacity, full_charge_capacity, cycle_count) = get_battery_capacity_wmi();

    // Calculate health if we have capacity data
    let health = if let (Some(design), Some(full)) = (design_capacity, full_charge_capacity) {
        if design > 0 {
            Some(((full as f32 / design as f32) * 100.0).min(100.0) as u8)
        } else {
            None
        }
    } else {
        None
    };

    // Get real-time power info
    let (voltage_mv, discharge_rate_mw) = get_battery_power_info();

    let design_voltage = battery_json["DesignVoltage"]
        .as_u64()
        .map(|v| v as u32)
        .or(voltage_mv); // Use current voltage as fallback

    Ok(BatteryInfo {
        is_present: true,
        state,
        percentage,
        time_remaining_secs,
        time_to_full_secs: None, // Could be calculated from charge rate
        design_capacity_mwh: design_capacity,
        full_charge_capacity_mwh: full_charge_capacity,
        health_percentage: health,
        cycle_count,
        technology,
        voltage_mv,
        design_voltage_mv: design_voltage,
        discharge_rate_mw,
        manufacturer: battery_json["Manufacturer"]
            .as_str()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
        serial_number: battery_json["SerialNumber"]
            .as_str()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
        manufacture_date: battery_json["ManufactureDate"]
            .as_str()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty()),
    })
}

/// Get battery capacity using BatteryStaticData and BatteryFullChargedCapacity WMI classes
///
/// These classes provide more accurate capacity information than Win32_Battery
fn get_battery_capacity_wmi() -> (Option<u32>, Option<u32>, Option<u32>) {
    use std::process::Command;

    // Try to get design capacity and full charge capacity
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$battery = Get-WmiObject -Namespace root\\wmi -Class BatteryStaticData | Select-Object -First 1; \
             $full = Get-WmiObject -Namespace root\\wmi -Class BatteryFullChargedCapacity | Select-Object -First 1; \
             $cycle = Get-WmiObject -Namespace root\\wmi -Class BatteryCycleCount -ErrorAction SilentlyContinue | Select-Object -First 1; \
             @{ \
                 DesignedCapacity = $battery.DesignedCapacity; \
                 FullChargedCapacity = $full.FullChargedCapacity; \
                 CycleCount = $cycle.CycleCount \
             } | ConvertTo-Json"
        ])
        .output()
        .ok();

    if let Some(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
            let design = json["DesignedCapacity"].as_u64().map(|c| c as u32);
            let full = json["FullChargedCapacity"].as_u64().map(|c| c as u32);
            let cycles = json["CycleCount"].as_u64().map(|c| c as u32);

            return (design, full, cycles);
        }
    }

    // Fallback: try older method
    (None, None, None)
}

/// Get real-time battery power information (voltage and discharge rate)
fn get_battery_power_info() -> (Option<u32>, Option<i32>) {
    use std::process::Command;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$status = Get-WmiObject -Namespace root\\wmi -Class BatteryStatus | Select-Object -First 1; \
             @{ \
                 Voltage = $status.Voltage; \
                 DischargeRate = $status.DischargeRate; \
                 ChargeRate = $status.ChargeRate \
             } | ConvertTo-Json"
        ])
        .output()
        .ok();

    if let Some(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
            let voltage = json["Voltage"].as_u64().map(|v| v as u32);

            // DischargeRate is positive when discharging, ChargeRate when charging
            let discharge = json["DischargeRate"].as_u64().map(|r| r as i32);
            let charge = json["ChargeRate"].as_u64().map(|r| -(r as i32));

            let rate = discharge.or(charge);

            return (voltage, rate);
        }
    }

    (None, None)
}

/// CPU Details returned from Windows-specific queries
pub struct CpuDetailsWindows {
    pub max_frequency_mhz: Option<u64>,
    pub turbo_enabled: Option<bool>,
    pub l2_cache_kb: Option<u32>,
    pub l3_cache_kb: Option<u32>,
    pub numa_nodes: Option<u32>,
}

/// Get CPU additional details using WMI
#[cfg(windows)]
pub fn get_cpu_details() -> Result<CpuDetailsWindows> {
    let wmi_con = WMIConnection::new()
        .map_err(|e| MscError::other(format!("Failed to connect to WMI: {}", e)))?;

    let processors: Vec<Win32Processor> = wmi_con
        .query()
        .map_err(|e| MscError::other(format!("WMI query failed: {}", e)))?;

    let processor = processors.first();

    let max_frequency = processor.and_then(|p| p.max_clock_speed.map(|f| f as u64));
    let turbo_enabled = processor.and_then(|p| p.turbo_mode_enabled);
    let l2_cache_kb = processor.and_then(|p| p.l2_cache_size);
    let l3_cache_kb = processor.and_then(|p| p.l3_cache_size);

    // Get NUMA node count
    let numa_nodes = get_numa_node_count().ok();

    Ok(CpuDetailsWindows {
        max_frequency_mhz: max_frequency,
        turbo_enabled,
        l2_cache_kb,
        l3_cache_kb,
        numa_nodes,
    })
}

#[cfg(not(windows))]
pub fn get_cpu_details() -> Result<CpuDetailsWindows> {
    Err(MscError::other(
        "get_cpu_details sólo está disponible en Windows",
    ))
}

/// Get NUMA node count using Windows API
fn get_numa_node_count() -> Result<u32> {
    use std::process::Command;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-WmiObject -Class Win32_NumaNode | Measure-Object).Count",
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to get NUMA nodes: {}", e)))?;

    let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let count = count_str.parse::<u32>().unwrap_or(1); // Default to 1 if parsing fails

    Ok(count)
}

/// Detect CPU instruction set support using CPUID
pub fn detect_cpu_instruction_sets() -> crate::core::system_info::types::CpuInstructionSets {
    use raw_cpuid::CpuId;

    let cpuid = CpuId::new();

    let mut instruction_sets = crate::core::system_info::types::CpuInstructionSets::default();

    // Check for instruction set support
    if let Some(features) = cpuid.get_feature_info() {
        instruction_sets.aes_ni = features.has_aesni();
        instruction_sets.sse4_2 = features.has_sse42();
    }

    if let Some(extended_features) = cpuid.get_extended_feature_info() {
        instruction_sets.avx2 = extended_features.has_avx2();
        instruction_sets.sha = extended_features.has_sha();

        // AVX-512 Foundation
        instruction_sets.avx512 = extended_features.has_avx512f();
    }

    if let Some(features) = cpuid.get_feature_info() {
        instruction_sets.fma3 = features.has_fma();
    }

    instruction_sets
}

/// AMD CPU Topology Information
pub struct AmdTopology {
    pub chiplet_count: Option<u32>,
    pub ccd_count: Option<u32>,
    pub ccx_count: Option<u32>,
}

/// Detect AMD CPU topology (chiplets, CCDs, CCXs)
pub fn detect_amd_topology(cpu_model: &str, physical_cores: usize) -> AmdTopology {
    let model_lower = cpu_model.to_lowercase();

    // Only for AMD Ryzen CPUs
    if !model_lower.contains("ryzen")
        && !model_lower.contains("threadripper")
        && !model_lower.contains("epyc")
    {
        return AmdTopology {
            chiplet_count: None,
            ccd_count: None,
            ccx_count: None,
        };
    }

    // Detect architecture and calculate topology
    let (chiplet_count, ccd_count, ccx_count) = if model_lower.contains("ryzen") {
        // Ryzen 9000/7000 series (Zen 4/5): 2 CCDs for high-end models
        if model_lower.contains("9950")
            || model_lower.contains("9900")
            || model_lower.contains("7950")
            || model_lower.contains("7900")
        {
            (Some(2), Some(2), Some(4)) // 2 chiplets, 2 CCDs, 4 CCXs (1 per CCD half)
        }
        // Ryzen 9700/9600/7700/7600: 1 CCD
        else if model_lower.contains("9700")
            || model_lower.contains("9600")
            || model_lower.contains("7700")
            || model_lower.contains("7600")
        {
            (Some(1), Some(1), Some(2)) // 1 chiplet, 1 CCD, 2 CCXs
        }
        // Ryzen 5000 series (Zen 3)
        else if model_lower.contains("5950") || model_lower.contains("5900") {
            (Some(2), Some(2), Some(2)) // 2 chiplets, 2 CCDs, 2 CCXs (Zen 3 unified)
        } else if model_lower.contains("5800")
            || model_lower.contains("5700")
            || model_lower.contains("5600")
        {
            (Some(1), Some(1), Some(1)) // 1 chiplet, 1 CCD, 1 CCX (Zen 3 unified)
        }
        // Ryzen 3000 series (Zen 2)
        else if model_lower.contains("3950") || model_lower.contains("3900") {
            (Some(2), Some(2), Some(4)) // 2 chiplets, 2 CCDs, 4 CCXs
        } else if physical_cores >= 6 {
            (Some(1), Some(1), Some(2)) // Default for mid-range
        } else {
            (Some(1), Some(1), Some(1)) // Default for low-end
        }
    } else if model_lower.contains("threadripper") {
        // Threadripper: estimate based on core count
        let estimated_ccds = (physical_cores / 8).max(1) as u32;
        (
            Some(estimated_ccds),
            Some(estimated_ccds),
            Some(estimated_ccds * 2),
        )
    } else {
        (None, None, None)
    };

    AmdTopology {
        chiplet_count,
        ccd_count,
        ccx_count,
    }
}

/// Get power plan information
pub fn get_power_plan_info() -> Result<PowerPlanInfo> {
    use std::process::Command;

    // Use powercfg to get the active power plan
    let output = Command::new("powercfg")
        .args(["/getactivescheme"])
        .output()
        .map_err(|e| MscError::other(format!("Failed to run powercfg: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Parse output: "Power Scheme GUID: <guid>  (Plan Name)"
    let active_plan_name = output_str
        .lines()
        .next()
        .and_then(|line| line.split('(').nth(1))
        .and_then(|s| s.split(')').next())
        .unwrap_or("Balanced")
        .trim()
        .to_string();

    let active_plan = match active_plan_name.to_lowercase().as_str() {
        s if s.contains("balanced") || s.contains("equilibrado") => PowerPlan::Balanced,
        s if s.contains("high") && s.contains("performance") => PowerPlan::HighPerformance,
        s if s.contains("best") && s.contains("performance") => PowerPlan::HighPerformance,
        s if s.contains("alto") && s.contains("rendimiento") => PowerPlan::HighPerformance,
        s if s.contains("power saver") || s.contains("ahorro") => PowerPlan::PowerSaver,
        s if s.contains("ultimate") => PowerPlan::UltimatePerformance,
        _ => PowerPlan::Custom(active_plan_name.clone()),
    };

    // Try to get Windows 11 Power Mode (Best Performance, Better Performance, Better Battery, etc.)
    let power_mode_output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Power\\User\\PowerSchemes' -Name ActiveOverlayAcPowerScheme -ErrorAction SilentlyContinue | Select-Object -ExpandProperty ActiveOverlayAcPowerScheme"
        ])
        .output()
        .ok();

    let power_mode = if let Some(output) = power_mode_output {
        let mode_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

        match mode_str.to_lowercase().as_str() {
            s if s.contains("54533251-82be-4824-96c1-47b60b740d00") => {
                Some(PowerMode::BestPerformance)
            }
            s if s.contains("ded574b5-45a0-4f42-8737-46345c09c238") => {
                Some(PowerMode::BestPerformance)
            } // Best Performance (alternative GUID)
            s if s.contains("3af9b8d9-7c97-431d-ad78-34a8bfea439f") => {
                Some(PowerMode::BetterPerformance)
            }
            s if s.contains("961cc777-2547-4f9d-8174-7d86181b8a7a") => {
                Some(PowerMode::BetterBattery)
            }
            s if s.contains("00000000-0000-0000-0000-000000000000") => {
                Some(PowerMode::BestPowerEfficiency)
            }
            _ => None,
        }
    } else {
        None
    };

    Ok(PowerPlanInfo {
        active_plan,
        power_mode,
        processor_min_state: None,
        processor_max_state: None,
        display_brightness_ac: None,
        display_brightness_dc: None,
        display_timeout_ac_secs: None,
        display_timeout_dc_secs: None,
        sleep_timeout_ac_secs: None,
        sleep_timeout_dc_secs: None,
    })
}

/// Detailed disk information from Windows
pub struct DiskDetailsWindows {
    pub disk_type: DiskType,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub firmware_version: Option<String>,
    pub bus_type: Option<crate::core::system_info::types::BusType>,
    pub interface_speed: Option<crate::core::system_info::types::InterfaceSpeed>,
    pub smart_status: Option<crate::core::system_info::types::SmartStatus>,
    pub temperature_celsius: Option<u32>,
    pub power_on_hours: Option<u64>,
    pub total_bytes_read: Option<u64>,
    pub total_bytes_written: Option<u64>,
}

/// Available storage expansion slots on the motherboard
#[derive(Debug, Clone)]
pub struct StorageSlots {
    pub sata_total: Option<u32>,
    pub sata_used: u32,
    pub sata_available: Option<u32>,
    pub m2_total: Option<u32>,
    pub m2_used: u32,
    pub m2_available: Option<u32>,
}

/// Detect bus type from PowerShell Get-PhysicalDisk output (REAL hardware detection)
fn detect_bus_type_from_powershell(
    bus_type_str: Option<&str>,
) -> Option<crate::core::system_info::types::BusType> {
    bus_type_str.and_then(|bus| match bus {
        "NVMe" => Some(BusType::NVMe),
        "SATA" | "ATA" | "ATAPI" => Some(BusType::SATA),
        "USB" | "Usb" => Some(BusType::USB),
        "SCSI" | "SAS" | "iSCSI" => Some(BusType::SCSI),
        _ => None,
    })
}

/// Get precise interface speed for NVMe drives by querying PCIe link information
///
/// This function queries the actual PCIe link speed and lane count from Windows,
/// rather than guessing based on model name.
fn get_interface_speed_precise(
    bus_type: &Option<crate::core::system_info::types::BusType>,
    device_id: Option<u64>,
) -> Option<crate::core::system_info::types::InterfaceSpeed> {
    use crate::core::system_info::types::{BusType, InterfaceSpeed};
    use std::process::Command;

    match bus_type {
        Some(BusType::NVMe) => {
            // For NVMe, try to get PCIe link information from device manager
            if let Some(dev_id) = device_id {
                // Query using Get-PnpDeviceProperty for PCIe link speed
                let output = Command::new("powershell")
                    .args([
                        "-NoProfile",
                        "-Command",
                        &format!(
                            "$disk = Get-PhysicalDisk -DeviceNumber {}; \
                             $pnp = Get-PnpDevice | Where-Object {{ $_.FriendlyName -eq $disk.FriendlyName }}; \
                             if ($pnp) {{ \
                                 $linkSpeed = Get-PnpDeviceProperty -InstanceId $pnp.InstanceId -KeyName 'DEVPKEY_PciDevice_CurrentLinkSpeed' -ErrorAction SilentlyContinue; \
                                 $linkWidth = Get-PnpDeviceProperty -InstanceId $pnp.InstanceId -KeyName 'DEVPKEY_PciDevice_CurrentLinkWidth' -ErrorAction SilentlyContinue; \
                                 @{{ Speed = $linkSpeed.Data; Width = $linkWidth.Data }} | ConvertTo-Json \
                             }}",
                            dev_id
                        )
                    ])
                    .output();

                if let Ok(output) = output {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
                        // Parse PCIe generation from link speed (GT/s)
                        // PCIe 3.0 = 8 GT/s, PCIe 4.0 = 16 GT/s, PCIe 5.0 = 32 GT/s
                        if let Some(speed) = json["Speed"].as_u64() {
                            let width = json["Width"].as_u64().unwrap_or(4);

                            // Determine PCIe generation
                            if speed >= 32000 && width >= 4 {
                                return Some(InterfaceSpeed::PCIe5x4);
                            } else if speed >= 16000 && width >= 4 {
                                return Some(InterfaceSpeed::PCIe4x4);
                            } else if speed >= 8000 {
                                if width >= 4 {
                                    return Some(InterfaceSpeed::PCIe3x4);
                                } else if width >= 2 {
                                    return Some(InterfaceSpeed::PCIe3x2);
                                }
                            }
                        }
                    }
                }
            }

            // Fallback: default to PCIe 3.0 x4 for NVMe
            Some(InterfaceSpeed::PCIe3x4)
        }
        Some(BusType::SATA) => {
            // Most modern SATA drives are SATA III (6 Gb/s)
            Some(InterfaceSpeed::SATA6Gbps)
        }
        Some(BusType::USB) => {
            // Default to USB 3.0 (can be improved with more detection)
            Some(InterfaceSpeed::USB3_5Gbps)
        }
        _ => None,
    }
}

/// Get detailed SMART data including temperature and read/write counters
///
/// Uses Windows Storage Management API (Get-StorageReliabilityCounter) to
/// retrieve comprehensive SMART attributes.
fn get_smart_data_detailed(device_id: u64) -> Result<SmartData> {
    use crate::core::system_info::types::SmartStatus;
    use std::process::Command;

    // Get SMART data using Get-StorageReliabilityCounter
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "$disk = Get-PhysicalDisk -DeviceNumber {}; \
                 $health = $disk.HealthStatus; \
                 $reliability = Get-StorageReliabilityCounter -PhysicalDisk $disk -ErrorAction SilentlyContinue; \
                 @{{ \
                     Health = $health; \
                     Temperature = $reliability.Temperature; \
                     ReadErrorsTotal = $reliability.ReadErrorsTotal; \
                     ReadErrorsCorrected = $reliability.ReadErrorsCorrected; \
                     WriteErrorsTotal = $reliability.WriteErrorsTotal; \
                     WriteErrorsCorrected = $reliability.WriteErrorsCorrected; \
                     Wear = $reliability.Wear; \
                     PowerOnHours = $reliability.PowerOnHours.TotalHours \
                 }} | ConvertTo-Json",
                device_id
            )
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to get SMART data: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
        let health_str = json["Health"].as_str();
        let smart_status = match health_str {
            Some("Healthy") => Some(SmartStatus::Healthy),
            Some("Warning") => Some(SmartStatus::Warning),
            Some("Unhealthy") => Some(SmartStatus::Critical),
            _ => Some(SmartStatus::Unknown),
        };

        let temperature = json["Temperature"].as_u64().map(|t| t as u32);

        let power_on_hours = json["PowerOnHours"].as_f64().map(|h| h as u64);

        // Calculate total bytes read/written from error counters (approximate)
        // Note: Not all drives report this accurately
        let bytes_read = json["ReadErrorsTotal"].as_u64();
        let bytes_written = json["WriteErrorsTotal"].as_u64();

        return Ok(SmartData {
            status: smart_status,
            temperature,
            power_on_hours,
            bytes_read,
            bytes_written,
        });
    }

    // Fallback to basic health check
    Ok(get_smart_data("").unwrap_or_default())
}

/// Get detailed disk information using modern Windows PowerShell APIs
///
/// Uses Get-PhysicalDisk instead of legacy Win32_DiskDrive for accurate hardware detection.
/// Provides REAL media type (HDD/SSD/NVMe), bus type, and SMART data including temperature.
pub fn get_disk_details(disk_name: &str) -> Result<DiskDetailsWindows> {
    use std::process::Command;

    // Use Get-PhysicalDisk (modern, accurate API) instead of Win32_DiskDrive (legacy)
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-PhysicalDisk | Select-Object DeviceId, FriendlyName, Model, MediaType, BusType, \
             HealthStatus, Size, SerialNumber, FirmwareVersion, Manufacturer | ConvertTo-Json",
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to run PowerShell: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    let json_value: serde_json::Value = serde_json::from_str(&output_str)
        .map_err(|e| MscError::other(format!("Failed to parse JSON: {}", e)))?;

    let disk_array = if json_value.is_array() {
        json_value.as_array().unwrap().clone()
    } else {
        vec![json_value.clone()]
    };

    // Try to match disk by name
    for disk_json in &disk_array {
        let friendly_name = disk_json["FriendlyName"].as_str();
        let model = disk_json["Model"].as_str();

        let name_to_match = friendly_name.or(model);

        if let Some(m) = name_to_match {
            if disk_name.contains(m) || m.contains(disk_name) {
                let device_id = disk_json["DeviceId"].as_u64();

                // Get REAL media type from PowerShell (not guessed!)
                let media_type_str = disk_json["MediaType"].as_str().unwrap_or("Unspecified");
                let bus_type_str = disk_json["BusType"].as_str();

                // Determine disk type from REAL hardware data
                let disk_type = match media_type_str {
                    "SSD" => DiskType::SSD,
                    "HDD" => DiskType::HDD,
                    "SCM" => DiskType::SSD, // Storage Class Memory (Intel Optane)
                    _ => {
                        // If MediaType is "Unspecified", check BusType
                        if let Some(bus) = bus_type_str {
                            if bus.contains("NVMe") {
                                DiskType::NVMe
                            } else {
                                DiskType::Unknown
                            }
                        } else {
                            DiskType::Unknown
                        }
                    }
                };

                // Get REAL bus type from PowerShell
                let bus_type = detect_bus_type_from_powershell(bus_type_str);

                // Get SMART data with temperature and counters
                let smart_data = if let Some(dev_id) = device_id {
                    get_smart_data_detailed(dev_id).unwrap_or_default()
                } else {
                    SmartData::default()
                };

                // Get precise interface speed (queries PCIe link for NVMe)
                let interface_speed = get_interface_speed_precise(&bus_type, device_id);

                // Extract manufacturer
                let manufacturer = disk_json["Manufacturer"]
                    .as_str()
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty() && s != "(Standard disk drives)")
                    .or_else(|| extract_manufacturer_from_model(model.unwrap_or("")));

                let serial_number = disk_json["SerialNumber"]
                    .as_str()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());

                let firmware_version = disk_json["FirmwareVersion"]
                    .as_str()
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty());

                return Ok(DiskDetailsWindows {
                    disk_type,
                    manufacturer,
                    model: model.map(|s| s.to_string()),
                    serial_number,
                    firmware_version,
                    bus_type,
                    interface_speed,
                    smart_status: smart_data.status,
                    temperature_celsius: smart_data.temperature,
                    power_on_hours: smart_data.power_on_hours,
                    total_bytes_read: smart_data.bytes_read,
                    total_bytes_written: smart_data.bytes_written,
                });
            }
        }
    }

    // Fallback if not found
    Ok(DiskDetailsWindows {
        disk_type: DiskType::Unknown,
        manufacturer: None,
        model: None,
        serial_number: None,
        firmware_version: None,
        bus_type: None,
        interface_speed: None,
        smart_status: None,
        temperature_celsius: None,
        power_on_hours: None,
        total_bytes_read: None,
        total_bytes_written: None,
    })
}

/// Extract manufacturer from model string
fn extract_manufacturer_from_model(model: &str) -> Option<String> {
    let model_lower = model.to_lowercase();

    let manufacturers = vec![
        ("samsung", "Samsung"),
        ("western digital", "Western Digital"),
        ("wd ", "Western Digital"),
        ("seagate", "Seagate"),
        ("crucial", "Crucial"),
        ("kingston", "Kingston"),
        ("sandisk", "SanDisk"),
        ("intel", "Intel"),
        ("micron", "Micron"),
        ("sk hynix", "SK hynix"),
        ("toshiba", "Toshiba"),
        ("corsair", "Corsair"),
        ("adata", "ADATA"),
        ("pny", "PNY"),
        ("gigabyte", "Gigabyte"),
        ("msi", "MSI"),
    ];

    for (pattern, name) in manufacturers {
        if model_lower.contains(pattern) {
            return Some(name.to_string());
        }
    }

    None
}

#[derive(Default)]
struct SmartData {
    status: Option<crate::core::system_info::types::SmartStatus>,
    temperature: Option<u32>,
    power_on_hours: Option<u64>,
    bytes_read: Option<u64>,
    bytes_written: Option<u64>,
}

/// Get SMART data for a disk (basic implementation)
fn get_smart_data(_model: &str) -> Result<SmartData> {
    use crate::core::system_info::types::SmartStatus;
    use std::process::Command;

    // Try to get SMART status using PowerShell
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-PhysicalDisk | Select-Object HealthStatus, OperationalStatus | ConvertTo-Json",
        ])
        .output();

    if let Ok(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);

        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&output_str) {
            let disk_data = if json_value.is_array() {
                json_value.as_array().and_then(|arr| arr.first())
            } else {
                Some(&json_value)
            };

            if let Some(data) = disk_data {
                let health_status = data["HealthStatus"].as_str();
                let smart_status = match health_status {
                    Some("Healthy") => Some(SmartStatus::Healthy),
                    Some("Warning") => Some(SmartStatus::Warning),
                    Some("Unhealthy") => Some(SmartStatus::Critical),
                    _ => Some(SmartStatus::Unknown),
                };

                return Ok(SmartData {
                    status: smart_status,
                    ..Default::default()
                });
            }
        }
    }

    // Could not get SMART data
    Ok(SmartData::default())
}

/// Get disk type information using PowerShell (legacy function for backward compatibility)
pub fn get_disk_type(disk_name: &str) -> Result<DiskType> {
    let details = get_disk_details(disk_name)?;
    Ok(details.disk_type)
}

/// Get available storage expansion slots (SATA and M.2)
///
/// This function detects how many storage slots are available on the motherboard
/// and how many are currently in use, helping users understand expansion capacity.
///
/// # Returns
/// * `Ok(StorageSlots)` - Information about available SATA and M.2 slots
/// * `Err(MscError)` - If detection fails
pub fn get_available_storage_slots() -> Result<StorageSlots> {
    use std::process::Command;

    // Count disks by bus type
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-PhysicalDisk | Select-Object BusType | ConvertTo-Json",
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to query storage devices: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    let mut sata_used = 0u32;
    let mut m2_used = 0u32;

    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&output_str) {
        let disk_array = if json_value.is_array() {
            json_value.as_array().unwrap()
        } else {
            &vec![json_value.clone()]
        };

        for disk in disk_array {
            if let Some(bus_type) = disk["BusType"].as_str() {
                match bus_type {
                    "SATA" | "ATA" | "ATAPI" => sata_used += 1,
                    "NVMe" => m2_used += 1,
                    _ => {}
                }
            }
        }
    }

    // Try to detect total SATA ports from motherboard (this is hardware-specific)
    // Most consumer motherboards have 4-8 SATA ports
    let sata_total = detect_total_sata_ports();

    // Try to detect total M.2 slots from motherboard
    // Most consumer motherboards have 1-3 M.2 slots
    let m2_total = detect_total_m2_slots();

    let sata_available = sata_total.map(|total| total.saturating_sub(sata_used));
    let m2_available = m2_total.map(|total| total.saturating_sub(m2_used));

    Ok(StorageSlots {
        sata_total,
        sata_used,
        sata_available,
        m2_total,
        m2_used,
        m2_available,
    })
}

/// Detect total SATA ports on motherboard
///
/// This is challenging because Windows doesn't directly expose this info.
/// We use heuristics based on chipset and SATA controllers.
fn detect_total_sata_ports() -> Option<u32> {
    use std::process::Command;

    // Query SATA controllers from WMI
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance -ClassName Win32_IDEController | Select-Object Name, Description | ConvertTo-Json"
        ])
        .output()
        .ok()?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&output_str) {
        let controller_array = if json_value.is_array() {
            json_value.as_array()?
        } else {
            &vec![json_value.clone()]
        };

        // Count SATA/AHCI controllers and estimate ports
        // Most SATA controllers have 6 ports, some have 4 or 8
        let controller_count = controller_array.len() as u32;

        if controller_count > 0 {
            // Conservative estimate: 4 ports per controller
            // (Most modern boards have 1-2 controllers with 4-6 ports each)
            return Some(controller_count * 4);
        }
    }

    // Fallback: typical consumer motherboard has 4-6 SATA ports
    Some(6)
}

/// Detect total M.2 slots on motherboard
///
/// M.2 slots are harder to detect via software. We use educated guesses based
/// on PCIe lane availability and common motherboard configurations.
fn detect_total_m2_slots() -> Option<u32> {
    use std::process::Command;

    // Try to detect M.2 slots from PCI devices
    // M.2 NVMe slots appear as PCIe devices
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-PnpDevice -Class 'SCSIAdapter' | Where-Object { $_.FriendlyName -like '*NVMe*' -or $_.FriendlyName -like '*M.2*' } | Measure-Object | Select-Object Count | ConvertTo-Json"
        ])
        .output()
        .ok()?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&output_str) {
        if let Some(count) = json_value["Count"].as_u64() {
            // This gives us a hint, but slots might be empty
            // Typical consumer boards: 1-3 M.2 slots
            // High-end boards: 2-4 M.2 slots
            if count > 0 {
                return Some((count as u32).max(2)); // At least 2 if we detected any
            }
        }
    }

    // Fallback: assume 2 M.2 slots (common on modern motherboards)
    Some(2)
}

// Type alias for GPU real-time metrics: (core_clock, mem_clock, temp, power, fan_speed)
type GpuRealtimeMetrics = (
    Option<u32>,
    Option<u32>,
    Option<u32>,
    Option<f32>,
    Option<u32>,
);

// ============================================================================
// DEPRECATED: Old hardcoded NVIDIA metrics function
// ============================================================================
// This function is kept for fallback when NVML is not available, but should
// NOT be used as primary source. NVML provides REAL data from hardware.
// The hardcoded values below are:
// - Not scalable (requires code changes for new GPUs)
// - Prone to errors (values can be wrong or outdated)
// - Imprecise matching (e.g., "5070" matches "5070 Ti", "5070 SUPER")
// - Assumes PCIe generation instead of reading real negotiated speed
//
// Use nvidia_nvml::get_nvidia_metrics_nvml() instead for professional-grade data.
// ============================================================================
// fn get_nvidia_metrics(...) { ... } // REMOVED - See nvidia_nvml.rs

/// Get AMD-specific GPU metrics
fn get_amd_metrics(model: &str) -> crate::core::system_info::types::AmdGpuMetrics {
    use crate::core::system_info::types::AmdGpuMetrics;

    let model_lower = model.to_lowercase();

    // RX 7000 Series (RDNA 3) - 5nm/6nm TSMC
    let (shader_count, compute_units, rop_count, tmu_count, infinity_cache, process_nm) =
        if model_lower.contains("7900 xtx") {
            (
                Some(6144),
                Some(96),
                Some(192),
                Some(384),
                Some(96),
                Some(5),
            )
        } else if model_lower.contains("7900 xt") {
            (
                Some(5376),
                Some(84),
                Some(192),
                Some(336),
                Some(80),
                Some(5),
            )
        } else if model_lower.contains("7900 gre") {
            (
                Some(5120),
                Some(80),
                Some(160),
                Some(320),
                Some(64),
                Some(5),
            )
        } else if model_lower.contains("7800 xt") {
            (Some(3840), Some(60), Some(96), Some(240), Some(64), Some(5))
        } else if model_lower.contains("7700 xt") {
            (Some(3456), Some(54), Some(64), Some(216), Some(48), Some(5))
        // 7600 XT and 7600 have same specs
        } else if model_lower.contains("7600 xt") || model_lower.contains("7600") {
            (Some(2048), Some(32), Some(64), Some(128), Some(32), Some(6))
        }
        // RX 6000 Series (RDNA 2) - 7nm TSMC
        // 6950 XT and 6900 XT have same specs
        else if model_lower.contains("6950 xt") || model_lower.contains("6900 xt") {
            (
                Some(5120),
                Some(80),
                Some(128),
                Some(320),
                Some(128),
                Some(7),
            )
        } else if model_lower.contains("6800 xt") {
            (
                Some(4608),
                Some(72),
                Some(128),
                Some(288),
                Some(128),
                Some(7),
            )
        } else if model_lower.contains("6800") {
            (
                Some(3840),
                Some(60),
                Some(96),
                Some(240),
                Some(128),
                Some(7),
            )
        } else if model_lower.contains("6700 xt") {
            (Some(2560), Some(40), Some(64), Some(160), Some(96), Some(7))
        } else if model_lower.contains("6600 xt") {
            (Some(2048), Some(32), Some(64), Some(128), Some(32), Some(7))
        } else if model_lower.contains("6600") {
            (Some(1792), Some(28), Some(64), Some(112), Some(32), Some(7))
        }
        // Integrated AMD Radeon Graphics (RDNA 2/3)
        else if model_lower.contains("radeon") && model_lower.contains("graphics") {
            // Ryzen AI 300 series (RDNA 3.5)
            if model_lower.contains("890m") {
                (Some(1024), Some(16), Some(32), Some(64), None, Some(4))
            } else if model_lower.contains("880m") {
                (Some(768), Some(12), Some(32), Some(48), None, Some(4))
            }
            // Ryzen 7000/6000 series (RDNA 2) - 780m and 680m have same specs
            else if model_lower.contains("780m") || model_lower.contains("680m") {
                (Some(768), Some(12), Some(32), Some(48), None, Some(6))
            } else {
                (Some(512), Some(8), Some(16), Some(32), None, Some(6))
            }
        } else {
            (None, None, None, None, None, None)
        };

    AmdGpuMetrics {
        shader_count,
        compute_units,
        rop_count,
        tmu_count,
        smartshift_power_watts: None, // Will try to get from real-time metrics
        infinity_cache_mb: infinity_cache,
        process_nm,
    }
}

/// Get real-time GPU metrics (temperature, clocks, fan speed, power draw)
fn get_gpu_realtime_metrics(_model: &str, vendor: &str) -> GpuRealtimeMetrics {
    // Try to get NVIDIA metrics using nvidia-smi
    if vendor.contains("NVIDIA") {
        if let Ok((core, mem, temp, power, fan)) = get_nvidia_realtime_metrics_by_index(0) {
            return (core, mem, temp, power, fan);
        }
    }

    // Try to get AMD metrics using PowerShell or WMI
    if vendor.contains("AMD") {
        if let Ok((core, mem, temp, power, fan)) = get_amd_realtime_metrics() {
            return (core, mem, temp, power, fan);
        }
    }

    // Fallback: no real-time data available
    (None, None, None, None, None)
}

/// Get NVIDIA real-time metrics using nvidia-smi for a specific GPU index
fn get_nvidia_realtime_metrics_by_index(gpu_index: u32) -> Result<GpuRealtimeMetrics> {
    use std::process::Command;

    // Try to run nvidia-smi with specific GPU index
    let output = Command::new("nvidia-smi")
        .args([
            &format!("--id={}", gpu_index),
            "--query-gpu=clocks.current.graphics,clocks.current.memory,temperature.gpu,power.draw,fan.speed",
            "--format=csv,noheader,nounits"
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to run nvidia-smi: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Parse CSV output: core_clock, mem_clock, temp, power, fan
    let parts: Vec<&str> = output_str.trim().split(',').collect();

    if parts.len() >= 5 {
        let core_clock = parts[0].trim().parse::<u32>().ok();
        let mem_clock = parts[1].trim().parse::<u32>().ok();
        let temp = parts[2].trim().parse::<u32>().ok();
        let power = parts[3].trim().parse::<f32>().ok();
        let fan = parts[4].trim().parse::<u32>().ok();

        return Ok((core_clock, mem_clock, temp, power, fan));
    }

    Ok((None, None, None, None, None))
}

/// Get NVIDIA VRAM size using nvidia-smi for a specific GPU index (fallback when NVML fails)
///
/// This function queries nvidia-smi for the total memory size, which is more
/// reliable than Win32_VideoController.AdapterRAM (which has a 4GB limit).
fn get_nvidia_vram_from_smi_by_index(gpu_index: u32) -> Option<u64> {
    use std::process::Command;

    // Query total memory in MiB for specific GPU
    let output = Command::new("nvidia-smi")
        .args([
            &format!("--id={}", gpu_index),
            "--query-gpu=memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mem_mib = output_str.trim().parse::<u64>().ok()?;

    // Convert MiB to bytes
    Some(mem_mib * 1024 * 1024)
}

/// Get AMD real-time metrics using PowerShell
fn get_amd_realtime_metrics() -> Result<GpuRealtimeMetrics> {
    // AMD metrics are harder to get without ADLX/ADL SDK
    // For now, return None - could be extended with WMI queries or third-party tools
    Ok((None, None, None, None, None))
}
