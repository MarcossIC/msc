use crate::core::system_info::types::{DdrType, MemoryModule};
use crate::error::{MscError, Result};
use serde::Deserialize;
use wmi::WMIConnection;

#[derive(Default)]
pub struct MemoryDetails {
    pub ddr_type: Option<DdrType>,
    pub speed_mhz: Option<u32>,
    pub modules: Vec<MemoryModule>,
    pub total_slots: Option<u32>,
    pub used_slots: Option<u32>,
    pub max_capacity_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32PhysicalMemoryArray {
    memory_devices: Option<u32>,
    // MaxCapacity is reported in KB.
    max_capacity: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32PhysicalMemory {
    capacity: Option<u64>,
    speed: Option<u32>,
    manufacturer: Option<String>,
    part_number: Option<String>,
    device_locator: Option<String>,
    #[serde(rename = "SMBIOSMemoryType")]
    smbios_type: Option<u16>,
}

fn ddr_from_smbios(value: Option<u16>) -> DdrType {
    match value {
        Some(18) => DdrType::DDR,
        Some(19) => DdrType::DDR2,
        Some(24) => DdrType::DDR3,
        Some(26) => DdrType::DDR4,
        Some(34) => DdrType::DDR5,
        _ => DdrType::Unknown,
    }
}

/// Get detailed memory information via direct WMI (root\CIMV2).
///
/// Migrated off PowerShell: two `Get-CimInstance` calls used to fork
/// `powershell.exe` twice, paying the engine-startup tax on every run. Both
/// `Win32_PhysicalMemory` and `Win32_PhysicalMemoryArray` are reachable
/// directly through the `wmi` crate on the default namespace.
pub fn get_memory_details() -> Result<MemoryDetails> {
    let wmi =
        WMIConnection::new().map_err(|e| MscError::other(format!("WMI connect failed: {e}")))?;

    let mem_modules: Vec<Win32PhysicalMemory> = wmi
        .raw_query(
            "SELECT Capacity, Speed, Manufacturer, PartNumber, DeviceLocator, SMBIOSMemoryType \
             FROM Win32_PhysicalMemory",
        )
        .map_err(|e| MscError::other(format!("WMI query failed: {e}")))?;

    let mut modules = Vec::new();
    let mut ddr_type = None;
    let mut speed_mhz = None;

    for mem in mem_modules {
        if ddr_type.is_none() {
            ddr_type = Some(ddr_from_smbios(mem.smbios_type));
        }

        if speed_mhz.is_none() {
            speed_mhz = mem.speed;
        }

        modules.push(MemoryModule {
            capacity_bytes: mem.capacity.unwrap_or(0),
            speed_mhz: mem.speed,
            manufacturer: mem.manufacturer,
            part_number: mem.part_number.map(|s| s.trim().to_string()),
            slot: mem.device_locator,
        });
    }

    // The array record is metadata about the slots; a missing row just means
    // we report None for capacity/slot totals rather than failing.
    let arrays: Vec<Win32PhysicalMemoryArray> = wmi
        .raw_query("SELECT MemoryDevices, MaxCapacity FROM Win32_PhysicalMemoryArray")
        .unwrap_or_default();
    let array_info = arrays.into_iter().next();

    Ok(MemoryDetails {
        ddr_type,
        speed_mhz,
        total_slots: array_info.as_ref().and_then(|a| a.memory_devices),
        used_slots: Some(modules.len() as u32),
        max_capacity_bytes: array_info.and_then(|a| a.max_capacity).map(|kb| kb * 1024),
        modules,
    })
}
