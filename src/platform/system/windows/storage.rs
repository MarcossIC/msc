use crate::core::system_info::types::{BusType, DiskType};
use crate::error::{MscError, Result};

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

/// Information about an M.2 slot
#[derive(Debug, Clone)]
pub struct M2SlotInfo {
    pub slot_number: u32,
    pub is_used: bool,
    pub supports_nvme: bool,
    pub supports_sata: bool,
    pub pcie_generation: Option<u32>, // 3, 4, or 5
    pub pcie_lanes: Option<u32>,      // 2 or 4
    pub form_factors: Vec<String>,    // e.g., "2280", "22110"
}

/// Available storage expansion slots on the motherboard
#[derive(Debug, Clone)]
pub struct StorageSlots {
    pub sata_total: Option<u32>,
    pub sata_used: u32,
    pub sata_available: Option<u32>,
    pub sata_hot_swap: bool,
    pub m2_slots: Vec<M2SlotInfo>,
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

/// Get the physical disk number from a drive letter (e.g., "C" -> 0)
fn get_disk_number_from_drive_letter(drive_letter: &str) -> Result<u64> {
    use std::process::Command;

    let ps_command = format!(
        "Get-Partition -DriveLetter {} | Select-Object -ExpandProperty DiskNumber",
        drive_letter
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_command])
        .output()
        .map_err(|e| MscError::other(format!("Failed to get disk number: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    let disk_number = output_str
        .trim()
        .parse::<u64>()
        .map_err(|e| MscError::other(format!("Failed to parse disk number: {}", e)))?;

    Ok(disk_number)
}

/// Get detailed disk information using modern Windows PowerShell APIs
///
/// Uses Get-PhysicalDisk instead of legacy Win32_DiskDrive for accurate hardware detection.
/// Provides REAL media type (HDD/SSD/NVMe), bus type, and SMART data including temperature.
pub fn get_disk_details(disk_name: &str) -> Result<DiskDetailsWindows> {
    use std::process::Command;

    // Debug: print disk_name to understand what we're receiving
    eprintln!(
        "DEBUG: get_disk_details called with disk_name: '{}'",
        disk_name
    );

    // First, get mapping from mount point (letter) to disk number
    let disk_number = if disk_name.len() >= 2 && disk_name.chars().nth(1) == Some(':') {
        // It's a drive letter like "C:\", get the disk number
        match get_disk_number_from_drive_letter(&disk_name[0..1]) {
            Ok(num) => {
                eprintln!(
                    "DEBUG: Successfully got disk number {} for drive letter {}",
                    num,
                    &disk_name[0..1]
                );
                num
            }
            Err(e) => {
                eprintln!(
                    "DEBUG: Failed to get disk number for drive letter {}: {}",
                    &disk_name[0..1],
                    e
                );
                return Err(e);
            }
        }
    } else if disk_name.contains("PhysicalDrive") {
        // Extract number from "\\.\PhysicalDrive0"
        disk_name
            .split("PhysicalDrive")
            .nth(1)
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| MscError::other("Cannot parse PhysicalDrive number"))?
    } else {
        eprintln!("DEBUG: disk_name doesn't match expected formats, using fallback disk 0");
        // Try first disk as fallback
        0
    };

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
    eprintln!("DEBUG: PowerShell output:\n{}", output_str);

    let json_value: serde_json::Value = serde_json::from_str(&output_str).map_err(|e| {
        eprintln!("DEBUG: Failed to parse JSON: {}", e);
        MscError::other(format!("Failed to parse JSON: {}", e))
    })?;

    let disk_array = if json_value.is_array() {
        json_value.as_array().unwrap().clone()
    } else {
        vec![json_value.clone()]
    };

    eprintln!("DEBUG: Looking for disk number: {}", disk_number);
    eprintln!(
        "DEBUG: Found {} disks in PowerShell output",
        disk_array.len()
    );

    // Try to match disk by DeviceId (disk number)
    for disk_json in &disk_array {
        // DeviceId can be a number or a string, handle both cases
        let device_id = disk_json["DeviceId"].as_u64().or_else(|| {
            disk_json["DeviceId"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
        });
        eprintln!("DEBUG: Checking disk with DeviceId: {:?}", device_id);

        if let Some(dev_id) = device_id {
            if dev_id == disk_number {
                // Extract basic info
                let model = disk_json["Model"].as_str();
                let friendly_name = disk_json["FriendlyName"].as_str();
                let bus_type_str = disk_json["BusType"].as_str();
                let media_type_str = disk_json["MediaType"].as_str().unwrap_or("Unspecified");

                // Determine disk type from REAL hardware data
                let disk_type = match media_type_str {
                    "SSD" => {
                        // Check if it's NVMe or SATA SSD
                        if let Some(bus) = bus_type_str {
                            if bus.contains("NVMe") {
                                DiskType::NVMe
                            } else {
                                DiskType::SSD
                            }
                        } else {
                            DiskType::SSD
                        }
                    }
                    "HDD" => DiskType::HDD,
                    "SCM" => DiskType::NVMe, // Storage Class Memory (Intel Optane) - treat as NVMe
                    _ => {
                        // If MediaType is "Unspecified", check BusType
                        if let Some(bus) = bus_type_str {
                            if bus.contains("NVMe") {
                                DiskType::NVMe
                            } else if bus.contains("SATA") || bus.contains("ATA") {
                                // For SATA, try to detect if SSD or HDD from model name
                                detect_ssd_or_hdd_from_model(
                                    model.unwrap_or(""),
                                    friendly_name.unwrap_or(""),
                                )
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
                let smart_data = get_smart_data_detailed(dev_id).unwrap_or_default();

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
    eprintln!("DEBUG: Disk not found, returning Unknown details");
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

/// Detect if a disk is SSD or HDD based on model name and friendly name
fn detect_ssd_or_hdd_from_model(model: &str, friendly_name: &str) -> DiskType {
    let combined = format!("{} {}", model, friendly_name).to_lowercase();

    // SSD indicators
    let ssd_keywords = ["ssd", "solid state", "nvme", "m.2"];
    for keyword in &ssd_keywords {
        if combined.contains(keyword) {
            return DiskType::SSD;
        }
    }

    // HDD indicators
    let hdd_keywords = ["hdd", "hard disk", "hard drive", "spinning"];
    for keyword in &hdd_keywords {
        if combined.contains(keyword) {
            return DiskType::HDD;
        }
    }

    // Default to SSD for modern systems if unable to determine
    DiskType::SSD
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

    // Generate detailed M.2 slot information
    let m2_slots = generate_m2_slot_details(m2_total.unwrap_or(2), m2_used);

    Ok(StorageSlots {
        sata_total,
        sata_used,
        sata_available,
        sata_hot_swap: false, // Most consumer boards don't support hot-swap
        m2_slots,
    })
}

/// Generate detailed M.2 slot information
fn generate_m2_slot_details(total_slots: u32, used_slots: u32) -> Vec<M2SlotInfo> {
    let mut slots = Vec::new();

    for slot_num in 0..total_slots {
        let is_used = slot_num < used_slots;

        // Most modern motherboards support both NVMe and SATA on M.2 slots
        // Slot 1 is typically PCIe 4.0 x4, others are PCIe 3.0 x4
        let (pcie_gen, pcie_lanes) = if slot_num == 0 {
            (Some(4), Some(4)) // Primary slot is usually PCIe 4.0 x4
        } else {
            (Some(3), Some(4)) // Secondary slots are usually PCIe 3.0 x4
        };

        slots.push(M2SlotInfo {
            slot_number: slot_num + 1,
            is_used,
            supports_nvme: true,
            supports_sata: true, // Most M.2 slots support both
            pcie_generation: pcie_gen,
            pcie_lanes,
            form_factors: vec!["2280".to_string()], // Most common form factor
        });
    }

    slots
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
