
use crate::error::{MscError, Result};
use crate::core::system_info::types::MotherboardInfo;

#[cfg(not(windows))]
pub fn get_motherboard_info() -> Result<MotherboardInfo> {
    Err(MscError::other(
        "get_motherboard_info sólo está disponible en Windows",
    ))
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