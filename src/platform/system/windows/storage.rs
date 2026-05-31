use crate::core::system_info::types::{BusType, DiskType};
use crate::error::{MscError, Result};
use serde::Deserialize;
use wmi::WMIConnection;

/// Detailed disk information from Windows
pub struct DiskDetailsWindows {
    pub disk_type: DiskType,
    /// Physical drive number (the WMI DeviceId / \\.\PhysicalDriveN index),
    /// used to join with the PCIe link map. None when detection fell back.
    pub device_number: Option<u32>,
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
    /// NVMe wear estimate (`PercentageUsed`, log page 02h). `None` for non-NVMe
    /// or when the SMART IOCTL isn't supported. May exceed 100 — 100 ≠ "dead".
    pub wear_percent: Option<u8>,
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
/// PCIe link speed (formerly via Get-PnpDeviceProperty) is not migrated yet —
/// there's no clean WMI equivalent. When the real link speed is unknown, NVMe
/// interface_speed is left `None` ("generation unknown") rather than assuming a
/// generation. A real reader (SetupAPI + DEVPKEY_PciDevice_CurrentLinkSpeed) is
/// the planned follow-up; until then we report the bus type without a fake gen.
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
            "SELECT DeviceId, HealthStatus, Temperature, PowerOnHours \
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

    // NVMe SMART/Health via native IOCTL (sub-ms, no admin) — the honest source
    // for wear, host bytes, temperature and power-on hours. Falls back to the WMI
    // counter per field when the IOCTL is unsupported (RAID/RST/vendor drivers).
    let nvme_health = if bus_type == Some(BusType::NVMe) {
        crate::platform::system::windows::nvme_smart::read_nvme_health(target)
    } else {
        None
    };

    // Temperature / power-on hours: prefer the IOCTL reading, fall back to WMI.
    let temperature = nvme_health
        .as_ref()
        .and_then(|h| h.composite_temp_c)
        .or_else(|| counter.as_ref().and_then(|c| c.temperature));
    let power_on_hours = nvme_health
        .as_ref()
        .map(|h| h.power_on_hours_u64())
        .or_else(|| {
            counter
                .as_ref()
                .and_then(|c| c.power_on_hours)
                .map(|h| h as u64)
        });

    // Host bytes come ONLY from the IOCTL (DataUnitRead/Written). WMI exposes
    // error COUNTS, not bytes — mapping ReadErrorsTotal/WriteErrorsTotal here was
    // a bug (mislabeled errors as "Data Written"). None when no IOCTL data.
    let bytes_read = nvme_health.as_ref().map(|h| h.bytes_read());
    let bytes_written = nvme_health.as_ref().map(|h| h.bytes_written());

    // NVMe wear estimate (PercentageUsed) — IOCTL only.
    let wear_percent = nvme_health.as_ref().map(|h| h.percentage_used);

    // No real PCIe link data available here → compute_interface_speed returns
    // None for NVMe (generation unknown) instead of fabricating PCIe 3.0 x4.
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
        device_number: Some(target),
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
        wear_percent,
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
        device_number: None,
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
        wear_percent: None,
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
                // `s` is the link speed in MT/s (GT/s × 1000). Map it to a PCIe
                // generation and keep the real lane width — together they model
                // ANY combo (e.g. Gen4 x2), unlike the old fixed variants.
                let gen = match s {
                    x if x >= 32000 => Some(5),
                    x if x >= 16000 => Some(4),
                    x if x >= 8000 => Some(3),
                    x if x >= 5000 => Some(2),
                    x if x >= 2500 => Some(1),
                    _ => None,
                };
                if let Some(g) = gen {
                    return Some(InterfaceSpeed::Pcie {
                        gen: g,
                        width: w as u8,
                    });
                }
            }
            // No real link data → DON'T fabricate a generation.
            // `None` means "NVMe, generation unknown"; the formatter renders just
            // "Interface: NVMe". Returning Some(PCIe3x4) here used to mislabel every
            // Gen4/Gen5 drive as Gen3 — a wrong value is worse than an absent one.
            // When the SetupAPI/DEVPKEY link-speed reader lands, pass real
            // (speed, width) and this branch becomes unreachable for detectable disks.
            None
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
    // Count disks by bus type via direct WMI (Storage Management namespace),
    // replacing a `Get-PhysicalDisk` PowerShell spawn. Here MSFT_PhysicalDisk
    // exposes BusType as a number (17=NVMe, 11=SATA), so we reuse
    // bus_type_from_num instead of matching PowerShell's string form.
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct BusOnly {
        bus_type: Option<u16>,
    }

    let mut sata_used = 0u32;
    let mut m2_used = 0u32;

    if let Ok(wmi) = WMIConnection::with_namespace_path("Root\\Microsoft\\Windows\\Storage") {
        let disks: Vec<BusOnly> = wmi
            .raw_query("SELECT BusType FROM MSFT_PhysicalDisk")
            .unwrap_or_default();

        for disk in disks {
            match bus_type_from_num(disk.bus_type) {
                Some(BusType::SATA) => sata_used += 1,
                Some(BusType::NVMe) => m2_used += 1,
                _ => {}
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

        // We do NOT read the actual M.2 slot PCIe capabilities from the firmware
        // yet (would need Win32_SystemSlot / SMBIOS type 9). Guessing "slot 0 is
        // Gen4, the rest Gen3" was pure fiction and directly contradicted the
        // disk's own reported interface. Leave it None until we read real data;
        // the formatter renders an honest "Unknown" instead of a made-up gen.
        let (pcie_gen, pcie_lanes): (Option<u32>, Option<u32>) = (None, None);

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
    // Query SATA/IDE controllers via direct WMI (root\CIMV2) instead of forking
    // PowerShell for a `Get-CimInstance`.
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32IdeController {
        name: Option<String>,
    }

    let controllers: Vec<Win32IdeController> = WMIConnection::new()
        .ok()
        .and_then(|wmi| wmi.raw_query("SELECT Name FROM Win32_IDEController").ok())
        .unwrap_or_default();

    // Count SATA/AHCI controllers and estimate ports.
    let controller_count = controllers.len() as u32;
    if controller_count > 0 {
        // Conservative estimate: 4 ports per controller
        // (Most modern boards have 1-2 controllers with 4-6 ports each)
        return Some(controller_count * 4);
    }

    // Fallback: typical consumer motherboard has 4-6 SATA ports
    Some(6)
}

/// Detect total M.2 slots on motherboard
///
/// M.2 slots are harder to detect via software. We use educated guesses based
/// on PCIe lane availability and common motherboard configurations.
fn detect_total_m2_slots() -> Option<u32> {
    // Detect M.2 NVMe adapters via direct WMI (root\CIMV2). The old
    // `Get-PnpDevice -Class 'SCSIAdapter'` maps to Win32_PnPEntity filtered by
    // PNPClass; FriendlyName maps to Name. Pushing the filter into WQL avoids a
    // full PnP scan and the PowerShell engine startup.
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32PnpEntity {
        name: Option<String>,
    }

    let devices: Vec<Win32PnpEntity> = WMIConnection::new()
        .ok()
        .and_then(|wmi| {
            wmi.raw_query(
                "SELECT Name FROM Win32_PnPEntity \
                 WHERE PNPClass = 'SCSIAdapter' \
                 AND (Name LIKE '%NVMe%' OR Name LIKE '%M.2%')",
            )
            .ok()
        })
        .unwrap_or_default();

    // This gives us a hint, but slots might be empty.
    // Typical consumer boards: 1-3 M.2 slots; high-end: 2-4.
    let count = devices.len() as u32;
    if count > 0 {
        return Some(count.max(2)); // At least 2 if we detected any
    }

    // Fallback: assume 2 M.2 slots (common on modern motherboards)
    Some(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::system_info::types::{BusType, InterfaceSpeed};

    // --- compute_interface_speed: honesty over assumption ---

    #[test]
    fn nvme_without_link_data_is_none_not_assumed_gen3() {
        // Regression guard: this used to return Some(PCIe3x4), mislabeling every
        // Gen4/Gen5 NVMe drive as Gen3. A wrong value is worse than an absent one.
        assert_eq!(
            compute_interface_speed(&Some(BusType::NVMe), None, None),
            None,
            "NVMe without real link data must be None (generation unknown)"
        );
    }

    #[test]
    fn nvme_gen3_link_data_is_detected() {
        assert_eq!(
            compute_interface_speed(&Some(BusType::NVMe), Some(8000), Some(4)),
            Some(InterfaceSpeed::Pcie { gen: 3, width: 4 })
        );
    }

    #[test]
    fn nvme_gen4_link_data_is_detected() {
        assert_eq!(
            compute_interface_speed(&Some(BusType::NVMe), Some(16000), Some(4)),
            Some(InterfaceSpeed::Pcie { gen: 4, width: 4 })
        );
    }

    #[test]
    fn nvme_gen5_link_data_is_detected() {
        assert_eq!(
            compute_interface_speed(&Some(BusType::NVMe), Some(32000), Some(4)),
            Some(InterfaceSpeed::Pcie { gen: 5, width: 4 })
        );
    }

    #[test]
    fn nvme_gen4_x2_is_representable() {
        // This is the WD_BLACK SN7100's real link. The OLD closed enum had no
        // PCIe4x2 variant and would have misclassified it as PCIe3x2 (wrong
        // generation). The {gen, width} model captures it exactly.
        assert_eq!(
            compute_interface_speed(&Some(BusType::NVMe), Some(16000), Some(2)),
            Some(InterfaceSpeed::Pcie { gen: 4, width: 2 })
        );
    }

    #[test]
    fn nvme_partial_link_data_is_none() {
        // Only speed, no width (or vice versa) → still can't be sure → None.
        assert_eq!(
            compute_interface_speed(&Some(BusType::NVMe), Some(16000), None),
            None
        );
    }

    #[test]
    fn sata_is_classified_from_bus_type() {
        // SATA III is the de-facto standard for the bus; this is a bus
        // classification, not a fabricated link-speed reading.
        assert_eq!(
            compute_interface_speed(&Some(BusType::SATA), None, None),
            Some(InterfaceSpeed::SATA6Gbps)
        );
    }

    #[test]
    fn unknown_bus_is_none() {
        assert_eq!(compute_interface_speed(&None, None, None), None);
    }

    // --- generate_m2_slot_details: no fabricated PCIe generations ---

    #[test]
    fn m2_slots_do_not_fabricate_pcie_generation() {
        // Regression guard: slot 0 was hardcoded PCIe 4.0 and the rest PCIe 3.0,
        // which directly contradicted the disk's own reported interface.
        let slots = generate_m2_slot_details(3, 1);
        assert_eq!(slots.len(), 3);
        for slot in &slots {
            assert_eq!(
                slot.pcie_generation, None,
                "M.2 slot PCIe generation must not be fabricated"
            );
            assert_eq!(
                slot.pcie_lanes, None,
                "M.2 slot PCIe lanes must not be fabricated"
            );
        }
    }

    #[test]
    fn m2_slots_track_used_count() {
        // The used/total bookkeeping is still meaningful and must survive the fix.
        let slots = generate_m2_slot_details(3, 1);
        assert!(slots[0].is_used);
        assert!(!slots[1].is_used);
        assert!(!slots[2].is_used);
    }
}
