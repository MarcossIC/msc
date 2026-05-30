use crate::core::system_info::types::MotherboardInfo;
use crate::error::Result;
#[cfg(windows)]
use serde::Deserialize;
#[cfg(windows)]
use wmi::WMIConnection;

#[cfg(not(windows))]
pub fn get_motherboard_info() -> Result<MotherboardInfo> {
    Err(crate::error::MscError::other(
        "get_motherboard_info sólo está disponible en Windows",
    ))
}

/// Baseboard + BIOS query result (5 fields).
#[cfg(windows)]
pub struct BaseboardBiosInfo {
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub version: Option<String>,
    pub bios_vendor: Option<String>,
    pub bios_version: Option<String>,
}

/// Query baseboard + BIOS via direct WMI (2 small queries on the same connection).
#[cfg(windows)]
pub fn get_baseboard_bios() -> BaseboardBiosInfo {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32BaseBoard {
        manufacturer: Option<String>,
        product: Option<String>,
        version: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32Bios {
        manufacturer: Option<String>,
        #[serde(rename = "SMBIOSBIOSVersion")]
        smbios_bios_version: Option<String>,
    }

    let empty = || BaseboardBiosInfo {
        manufacturer: None,
        product: None,
        version: None,
        bios_vendor: None,
        bios_version: None,
    };

    let wmi = match WMIConnection::new() {
        Ok(w) => w,
        Err(_) => return empty(),
    };

    let baseboards: Vec<Win32BaseBoard> = wmi.query().unwrap_or_default();
    let bioses: Vec<Win32Bios> = wmi.query().unwrap_or_default();

    let bb = baseboards.into_iter().next();
    let bi = bioses.into_iter().next();

    let nz = |s: Option<String>| s.filter(|v| !v.is_empty());

    BaseboardBiosInfo {
        manufacturer: nz(bb.as_ref().and_then(|b| b.manufacturer.clone())),
        product: nz(bb.as_ref().and_then(|b| b.product.clone())),
        version: nz(bb.as_ref().and_then(|b| b.version.clone())),
        bios_vendor: nz(bi.as_ref().and_then(|b| b.manufacturer.clone())),
        bios_version: nz(bi.as_ref().and_then(|b| b.smbios_bios_version.clone())),
    }
}

/// Get motherboard information using PowerShell (orchestrates 4 sub-queries).
#[cfg(windows)]
pub fn get_motherboard_info() -> Result<MotherboardInfo> {
    let bb = get_baseboard_bios();
    let chipset = detect_chipset();
    let tpm_version = detect_tpm_version();
    let dimm_slots = get_dimm_slot_count();

    Ok(MotherboardInfo {
        manufacturer: bb.manufacturer,
        product: bb.product,
        version: bb.version,
        bios_vendor: bb.bios_vendor,
        bios_version: bb.bios_version,
        chipset,
        tpm_version,
        dimm_slots,
        pcie_slots: None, // Would require Win32_SystemSlot query
        m2_slots_total: None,
        m2_slots_used: None,
    })
}

/// Detect motherboard chipset via direct WMI.
///
/// Queries Win32_PnPEntity with WHERE filter (no full-table scan) to find
/// chipset/SMBus/LPC controller devices.
pub fn detect_chipset() -> Option<String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32PnpEntity {
        name: Option<String>,
    }

    let wmi = WMIConnection::new().ok()?;

    // raw_query lets us push the WHERE down to WMI, avoiding a full PnP scan.
    let entities: Vec<Win32PnpEntity> = wmi
        .raw_query(
            "SELECT Name FROM Win32_PnPEntity \
             WHERE Name LIKE '%Chipset%' OR Name LIKE '%SMBus%' OR Name LIKE '%LPC Controller%'",
        )
        .ok()?;

    let first_name = entities.into_iter().find_map(|e| e.name)?;
    if first_name.is_empty() {
        return None;
    }
    Some(extract_chipset_name(&first_name))
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
                chipset = chipset.replace("(R)", "").trim().to_string();
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

/// Cache key for TPM version. TPM hardware does not change at runtime, so we
/// cache for 30 days to avoid the ~5s WMI security-namespace cold start.
const TPM_CACHE_KEY: &str = "tpm_version";
const TPM_CACHE_TTL_DAYS: u64 = 30;

/// Detect TPM version using PowerShell, with disk-cache (30-day TTL).
///
/// First call queries `root\CIMV2\Security\MicrosoftTpm` (slow). Subsequent
/// calls within TTL read from the cache (microseconds), even when the result
/// is `None` (TPM unavailable / no admin / disabled). Use `--clear-cache` if
/// the hardware state changed.
pub fn detect_tpm_version() -> Option<crate::core::system_info::types::TpmVersion> {
    use crate::core::system_info::cache;
    use crate::core::system_info::types::TpmVersion;

    // Cache stores the full Option so negatives are remembered too.
    if let Some(cached) = cache::get::<Option<TpmVersion>>(TPM_CACHE_KEY) {
        return cached;
    }

    let result = query_tpm_version_uncached();
    cache::set(TPM_CACHE_KEY, &result, TPM_CACHE_TTL_DAYS);
    result
}

fn query_tpm_version_uncached() -> Option<crate::core::system_info::types::TpmVersion> {
    use crate::core::system_info::types::TpmVersion;
    use std::process::Command;

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

/// Get the number of DIMM slots from Win32_PhysicalMemoryArray via direct WMI.
pub fn get_dimm_slot_count() -> Option<u32> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32PhysicalMemoryArray {
        memory_devices: Option<u32>,
    }

    let wmi = WMIConnection::new().ok()?;
    let arrays: Vec<Win32PhysicalMemoryArray> = wmi.query().ok()?;
    arrays.into_iter().next()?.memory_devices
}
