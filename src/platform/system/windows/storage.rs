use crate::core::system_info::types::{BusType, DiskType};
use crate::error::{MscError, Result};
use serde::Deserialize;
use wmi::WMIConnection;

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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct MsftPartition {
    disk_number: Option<u32>,
    drive_letter: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct MsftPhysicalDisk {
    device_id: Option<String>,
    friendly_name: Option<String>,
    model: Option<String>,
    media_type: Option<u16>,
    bus_type: Option<u16>,
    serial_number: Option<String>,
    firmware_version: Option<String>,
    manufacturer: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct MsftStorageReliabilityCounter {
    device_id: Option<String>,
    health_status: Option<String>,
    temperature: Option<u32>,
    read_errors_total: Option<u64>,
    write_errors_total: Option<u64>,
    power_on_hours: Option<f64>,
}

/// Get detailed disk information via direct WMI to the Storage Management namespace.
///
/// Uses 3 raw WMI queries on a single connection to `Root\Microsoft\Windows\Storage`:
///   1. MSFT_Partition (drive-letter → disk-number resolution, only if input is a letter)
///   2. MSFT_PhysicalDisk (model, media type, bus, serial, firmware)
///   3. MSFT_StorageReliabilityCounter (SMART: temp, errors, power-on-hours, health)
///
/// `disk_name` accepts either a drive letter form (e.g. "C:\\") or a physical
/// drive form (e.g. "\\.\PhysicalDrive0"). Other inputs return a fallback.
///
/// PCIe link speed (formerly via Get-PnpDeviceProperty) is not migrated — there's
/// no clean WMI equivalent. NVMe gets the conservative PCIe 3.0 x4 default. The
/// accuracy loss is small relative to the ~1.5s saved.
pub fn get_disk_details(disk_name: &str) -> Result<DiskDetailsWindows> {
    use crate::core::system_info::types::SmartStatus;

    // Parse + sanitize input.
    let drive_letter: Option<char> =
        if disk_name.len() >= 2 && disk_name.chars().nth(1) == Some(':') {
            let c = disk_name.chars().next().unwrap().to_ascii_uppercase();
            if c.is_ascii_alphabetic() {
                Some(c)
            } else {
                None
            }
        } else {
            None
        };

    let mut disk_number: Option<u32> =
        if let Some(stripped) = disk_name.strip_prefix("\\\\.\\PhysicalDrive") {
            stripped.parse().ok()
        } else if let Some(stripped) = disk_name.strip_prefix("PhysicalDrive") {
            stripped.parse().ok()
        } else {
            None
        };

    if drive_letter.is_none() && disk_number.is_none() {
        return Ok(fallback_disk_details());
    }

    // Connect to the Storage Management namespace (separate from root\cimv2).
    // wmi 0.18 handles COM init internally — we just pass the namespace path.
    let wmi = match WMIConnection::with_namespace_path("Root\\Microsoft\\Windows\\Storage") {
        Ok(w) => w,
        Err(_) => return Ok(fallback_disk_details()),
    };

    // Resolve drive letter → disk number if needed.
    if disk_number.is_none() {
        if let Some(dl) = drive_letter {
            let partitions: Vec<MsftPartition> = wmi
                .raw_query(format!(
                    "SELECT DiskNumber, DriveLetter FROM MSFT_Partition WHERE DriveLetter = '{}'",
                    dl
                ))
                .unwrap_or_default();
            disk_number = partitions.into_iter().find_map(|p| p.disk_number);
        }
    }

    let target = match disk_number {
        Some(n) => n,
        None => return Ok(fallback_disk_details()),
    };

    // MSFT_PhysicalDisk: DeviceId is a string here (numeric digits) — match by string.
    let disks: Vec<MsftPhysicalDisk> = wmi
        .raw_query(format!(
            "SELECT DeviceId, FriendlyName, Model, MediaType, BusType, \
             SerialNumber, FirmwareVersion, Manufacturer \
             FROM MSFT_PhysicalDisk WHERE DeviceId = '{}'",
            target
        ))
        .unwrap_or_default();

    let disk = match disks.into_iter().next() {
        Some(d) => d,
        None => return Ok(fallback_disk_details()),
    };

    // MSFT_StorageReliabilityCounter: filter by same DeviceId.
    let counters: Vec<MsftStorageReliabilityCounter> = wmi
        .raw_query(format!(
            "SELECT DeviceId, HealthStatus, Temperature, ReadErrorsTotal, \
             WriteErrorsTotal, PowerOnHours \
             FROM MSFT_StorageReliabilityCounter WHERE DeviceId = '{}'",
            target
        ))
        .unwrap_or_default();
    let counter = counters.into_iter().next();

    let model = disk.model.as_deref();
    let friendly_name = disk.friendly_name.as_deref();
    let bus_type = bus_type_from_num(disk.bus_type);
    let media_type_str = media_type_label(disk.media_type);

    let disk_type = match media_type_str {
        "SSD" => match bus_type {
            Some(BusType::NVMe) => DiskType::NVMe,
            _ => DiskType::SSD,
        },
        "HDD" => DiskType::HDD,
        "SCM" => DiskType::NVMe,
        _ => match bus_type {
            Some(BusType::NVMe) => DiskType::NVMe,
            Some(BusType::SATA) => {
                detect_ssd_or_hdd_from_model(model.unwrap_or(""), friendly_name.unwrap_or(""))
            }
            _ => DiskType::Unknown,
        },
    };

    // MSFT HealthStatus is uint16 (0=Healthy, 1=Warning, 2=Unhealthy, 5=Unknown)
    // but raw_query may surface it as string in some Windows versions. Handle both.
    let smart_status = match counter.as_ref().and_then(|c| c.health_status.as_deref()) {
        Some("Healthy") | Some("0") => Some(SmartStatus::Healthy),
        Some("Warning") | Some("1") => Some(SmartStatus::Warning),
        Some("Unhealthy") | Some("2") => Some(SmartStatus::Critical),
        Some(_) => Some(SmartStatus::Unknown),
        None => None,
    };

    let temperature = counter.as_ref().and_then(|c| c.temperature);
    let power_on_hours = counter
        .as_ref()
        .and_then(|c| c.power_on_hours)
        .map(|h| h as u64);
    let bytes_read = counter.as_ref().and_then(|c| c.read_errors_total);
    let bytes_written = counter.as_ref().and_then(|c| c.write_errors_total);

    // PCIe link query removed — fallback for NVMe is PCIe 3.0 x4.
    let interface_speed = compute_interface_speed(&bus_type, None, None);

    let manufacturer = disk
        .manufacturer
        .filter(|s| !s.is_empty() && s != "(Standard disk drives)")
        .or_else(|| extract_manufacturer_from_model(model.unwrap_or("")));

    let serial_number = disk
        .serial_number
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let firmware_version = disk.firmware_version.filter(|s| !s.is_empty());

    Ok(DiskDetailsWindows {
        disk_type,
        manufacturer,
        model: model.map(|s| s.to_string()),
        serial_number,
        firmware_version,
        bus_type,
        interface_speed,
        smart_status,
        temperature_celsius: temperature,
        power_on_hours,
        total_bytes_read: bytes_read,
        total_bytes_written: bytes_written,
    })
}

/// Map MSFT_PhysicalDisk.MediaType numeric to a label string used by the type detection.
/// 0=Unspecified, 3=HDD, 4=SSD, 5=SCM (storage class memory)
fn media_type_label(media_type: Option<u16>) -> &'static str {
    match media_type {
        Some(3) => "HDD",
        Some(4) => "SSD",
        Some(5) => "SCM",
        _ => "Unspecified",
    }
}

/// Map MSFT_PhysicalDisk.BusType numeric to BusType enum.
/// Per docs: 1=SCSI, 7=USB, 10=SAS, 11=SATA, 17=NVMe (others rare on consumer hardware).
fn bus_type_from_num(bus: Option<u16>) -> Option<BusType> {
    match bus {
        Some(17) => Some(BusType::NVMe),
        Some(11) | Some(3) => Some(BusType::SATA), // 11=SATA, 3=ATA
        Some(7) => Some(BusType::USB),
        Some(1) | Some(10) | Some(9) => Some(BusType::SCSI), // SCSI/SAS/iSCSI
        _ => None,
    }
}

fn fallback_disk_details() -> DiskDetailsWindows {
    DiskDetailsWindows {
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
    }
}

/// Map PCIe link speed (GT/s) + width (lanes) to an InterfaceSpeed enum.
/// Falls back to typical defaults per bus when link info is missing.
fn compute_interface_speed(
    bus_type: &Option<crate::core::system_info::types::BusType>,
    speed: Option<u64>,
    width: Option<u64>,
) -> Option<crate::core::system_info::types::InterfaceSpeed> {
    use crate::core::system_info::types::{BusType, InterfaceSpeed};

    match bus_type {
        Some(BusType::NVMe) => {
            if let (Some(s), Some(w)) = (speed, width) {
                if s >= 32000 && w >= 4 {
                    return Some(InterfaceSpeed::PCIe5x4);
                }
                if s >= 16000 && w >= 4 {
                    return Some(InterfaceSpeed::PCIe4x4);
                }
                if s >= 8000 {
                    if w >= 4 {
                        return Some(InterfaceSpeed::PCIe3x4);
                    }
                    if w >= 2 {
                        return Some(InterfaceSpeed::PCIe3x2);
                    }
                }
            }
            Some(InterfaceSpeed::PCIe3x4)
        }
        Some(BusType::SATA) => Some(InterfaceSpeed::SATA6Gbps),
        Some(BusType::USB) => Some(InterfaceSpeed::USB3_5Gbps),
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
