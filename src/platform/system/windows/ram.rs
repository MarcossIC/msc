use serde::Deserialize;
use crate::error::{Result};
use crate::core::system_info::types::{DdrType, MemoryModule};
use super::core::run_powershell_json;

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

#[derive(Debug, Deserialize)]
struct PhysicalMemoryArrayPs {
    #[serde(rename = "MemoryDevices")]
    memory_devices: Option<u32>,
    #[serde(rename = "MaxCapacity")]
    max_capacity_kb: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PhysicalMemoryPs {
    #[serde(rename = "Capacity")]
    capacity: u64,
    #[serde(rename = "Speed")]
    speed: Option<u32>,
    #[serde(rename = "Manufacturer")]
    manufacturer: Option<String>,
    #[serde(rename = "PartNumber")]
    part_number: Option<String>,
    #[serde(rename = "DeviceLocator")]
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

/// Get detailed memory information using PowerShell
pub fn get_memory_details() -> Result<MemoryDetails> {
    let mut modules = Vec::new();
    let mut ddr_type = None;
    let mut speed_mhz = None;

    let mem_data: serde_json::Value = run_powershell_json(
        "Get-CimInstance Win32_PhysicalMemory \
         | Select Capacity, Speed, Manufacturer, PartNumber, DeviceLocator, SMBIOSMemoryType \
         | ConvertTo-Json",
    )?;

    let mem_modules: Vec<PhysicalMemoryPs> = match mem_data {
        serde_json::Value::Array(arr) => serde_json::from_value(arr.into())?,
        value => vec![serde_json::from_value(value)?],
    };

    for mem in mem_modules {
        if ddr_type.is_none() {
            ddr_type = Some(ddr_from_smbios(mem.smbios_type));
        }

        if speed_mhz.is_none() {
            speed_mhz = mem.speed;
        }

        modules.push(MemoryModule {
            capacity_bytes: mem.capacity,
            speed_mhz: mem.speed,
            manufacturer: mem.manufacturer,
            part_number: mem.part_number.map(|s| s.trim().to_string()),
            slot: mem.device_locator,
        });
    }

    let array_data: serde_json::Value = run_powershell_json(
        "Get-CimInstance Win32_PhysicalMemoryArray \
         | Select MemoryDevices, MaxCapacity \
         | ConvertTo-Json",
    )?;

    let array_info: PhysicalMemoryArrayPs = match array_data {
        serde_json::Value::Array(arr) => serde_json::from_value(arr[0].clone())?,
        value => serde_json::from_value(value)?,
    };

    Ok(MemoryDetails {
        ddr_type,
        speed_mhz,
        total_slots: array_info.memory_devices,
        used_slots: Some(modules.len() as u32),
        max_capacity_bytes: array_info.max_capacity_kb.map(|kb| kb * 1024),
        modules,
    })
}