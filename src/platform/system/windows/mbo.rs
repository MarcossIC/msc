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
    let (m2_slots_total, m2_slots_used) = get_m2_slots();

    Ok(MotherboardInfo {
        manufacturer: bb.manufacturer,
        product: bb.product,
        version: bb.version,
        bios_vendor: bb.bios_vendor,
        bios_version: bb.bios_version,
        chipset,
        tpm_version,
        secure_boot: detect_secure_boot(),
        dimm_slots,
        pcie_slots: None, // Would require Win32_SystemSlot query
        m2_slots_total,
        m2_slots_used,
    })
}

/// Detect M.2 slots via `Win32_SystemSlot` (direct WMI, root\CIMV2).
///
/// Returns `(total, used)`. Both are `None` when the firmware exposes no M.2
/// slot — **never fabricated**. This is the honest gotcha worth knowing: the
/// class is filled from **SMBIOS Type 9** ("System Slots"), and many laptop OEMs
/// simply don't populate Type 9 for soldered/internal M.2 slots, so this query
/// legitimately comes back empty on a lot of notebooks (desktops/server boards
/// are far more reliable). When that happens we report nothing rather than guess.
///
/// `CurrentUsage` enum (per Microsoft docs — NOT 1/2 as folklore claims):
///   0 Reserved · 1 Other · 2 Unknown · 3 Available · **4 In use**.
fn get_m2_slots() -> (Option<u32>, Option<u32>) {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32SystemSlot {
        slot_designation: Option<String>,
        current_usage: Option<u16>,
    }

    let wmi = match WMIConnection::new() {
        Ok(w) => w,
        Err(_) => return (None, None),
    };

    let slots: Vec<Win32SystemSlot> = match wmi
        .raw_query("SELECT SlotDesignation, CurrentUsage FROM Win32_SystemSlot")
    {
        Ok(s) => s,
        Err(_) => return (None, None),
    };

    // SMBIOS Type 9 has no dedicated "M.2" connector enum, so we match the OEM's
    // free-form SlotDesignation string ("M.2", "M2", "NVMe").
    let is_m2 = |d: &str| {
        let u = d.to_ascii_uppercase();
        u.contains("M.2") || u.contains("M2") || u.contains("NVME")
    };

    let mut total = 0u32;
    let mut used = 0u32;
    for slot in &slots {
        let Some(desig) = slot.slot_designation.as_deref() else {
            continue;
        };
        if !is_m2(desig) {
            continue;
        }
        total += 1;
        // Only CurrentUsage == 4 ("In use") counts as occupied. Available (3),
        // Unknown (2) or unset don't — we'd rather undercount than overclaim.
        if slot.current_usage == Some(4) {
            used += 1;
        }
    }

    if total == 0 {
        return (None, None); // No M.2 slot exposed by firmware — don't invent one.
    }

    (Some(total), Some(used))
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

/// Detect TPM version via the native **TBS (TPM Base Services)** API.
///
/// `Tbsi_GetDeviceInfo` (tbs.dll) hands back a `TPM_DEVICE_INFO` directly — no
/// admin, no WMI. This replaced the old `root\CIMV2\Security\MicrosoftTpm` WMI
/// query, whose security-namespace cold start cost ~5s and dominated the
/// uncached wall-clock. TBS answers in well under a millisecond, so the former
/// 30-day disk cache was removed deliberately: it bought no speed and could
/// report a stale version for up to 30 days after a BIOS/firmware change — a
/// direct hit to the project's "never fabricate" rule for zero gain.
pub fn detect_tpm_version() -> Option<crate::core::system_info::types::TpmVersion> {
    let raw = query_tpm_device_info()?;
    Some(map_tpm_version(raw))
}

/// Fetch the raw `tpmVersion` field from TBS, or `None` when no TPM is present
/// or its device info can't be read (TBS returns a non-zero result code).
///
/// The `unsafe` is isolated here so the value mapping ([`map_tpm_version`])
/// stays pure and unit-testable without touching hardware.
fn query_tpm_device_info() -> Option<u32> {
    use windows_sys::Win32::System::TpmBaseServices::{
        Tbsi_GetDeviceInfo, TBS_SUCCESS, TPM_DEVICE_INFO,
    };

    let mut info = TPM_DEVICE_INFO::default();
    let size = std::mem::size_of::<TPM_DEVICE_INFO>() as u32;

    // SAFETY: `info` is a zero-initialized, correctly aligned #[repr(C)]
    // TPM_DEVICE_INFO and `size` is exactly its byte length, which is what
    // Tbsi_GetDeviceInfo requires. The call only writes into the buffer, and
    // only on success; we read `info` solely on the TBS_SUCCESS path below, so
    // it is never read while uninitialized.
    let rc = unsafe {
        Tbsi_GetDeviceInfo(
            size,
            &mut info as *mut TPM_DEVICE_INFO as *mut core::ffi::c_void,
        )
    };

    if rc != TBS_SUCCESS {
        return None; // No TPM / not readable — never fabricated.
    }

    Some(info.tpmVersion)
}

/// Pure mapping from the TBS `TPM_DEVICE_INFO.tpmVersion` field to our enum.
///
/// `1` (TPM_VERSION_12) and `2` (TPM_VERSION_20) map to the concrete versions;
/// anything else — including `0` (TPM_VERSION_UNKNOWN) — degrades to `Unknown`,
/// never a guessed version. No I/O, so it is covered by golden tests.
fn map_tpm_version(raw: u32) -> crate::core::system_info::types::TpmVersion {
    use crate::core::system_info::types::TpmVersion;
    use windows_sys::Win32::System::TpmBaseServices::{TPM_VERSION_12, TPM_VERSION_20};

    match raw {
        TPM_VERSION_12 => TpmVersion::V1_2,
        TPM_VERSION_20 => TpmVersion::V2_0,
        _ => TpmVersion::Unknown,
    }
}

/// Raw outcome of probing the Secure Boot registry state, kept SEPARATE from the
/// classification so the mapping below stays pure and golden-testable without a
/// real registry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SecureBootProbe {
    /// `UEFISecureBootEnabled` read as a DWORD (1 = on, 0 = off per Microsoft).
    Value(u32),
    /// The `SecureBoot\State` key or its value is absent — legacy BIOS / CSM,
    /// which has no UEFI Secure Boot at all.
    NotPresent,
    /// The registry couldn't be read (unexpected) — we refuse to guess.
    Unreadable,
}

/// Pure mapping from a [`SecureBootProbe`] to the reported status.
///
/// `Value(1)` → Enabled; any other value → Disabled (the DWORD is 0/1 in
/// practice, and anything non-1 is honestly "not enabled"). `NotPresent` means
/// the machine booted legacy BIOS, so Secure Boot is `Unsupported`, **never** a
/// misleading `Disabled`. `Unreadable` → `None`: undetermined, not fabricated.
/// No I/O — covered by golden tests.
pub fn classify_secure_boot(
    probe: SecureBootProbe,
) -> Option<crate::core::system_info::types::SecureBootStatus> {
    use crate::core::system_info::types::SecureBootStatus;
    match probe {
        SecureBootProbe::Value(1) => Some(SecureBootStatus::Enabled),
        SecureBootProbe::Value(_) => Some(SecureBootStatus::Disabled),
        SecureBootProbe::NotPresent => Some(SecureBootStatus::Unsupported),
        SecureBootProbe::Unreadable => None,
    }
}

/// Read the UEFI Secure Boot state from the registry — no admin, no WMI.
///
/// `HKLM\SYSTEM\CurrentControlSet\Control\SecureBoot\State\UEFISecureBootEnabled`
/// is a DWORD (1 = enabled). The `State` subkey only exists on UEFI systems, so
/// its absence means the machine booted legacy BIOS/CSM — reported as
/// `Unsupported`, never a misleading "Disabled". The registry I/O is isolated
/// here so [`classify_secure_boot`] stays pure and unit-testable.
#[cfg(windows)]
pub fn detect_secure_boot() -> Option<crate::core::system_info::types::SecureBootStatus> {
    use std::io::ErrorKind;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let probe = match hklm.open_subkey(r"SYSTEM\CurrentControlSet\Control\SecureBoot\State") {
        Ok(key) => match key.get_value::<u32, _>("UEFISecureBootEnabled") {
            Ok(v) => SecureBootProbe::Value(v),
            Err(e) if e.kind() == ErrorKind::NotFound => SecureBootProbe::NotPresent,
            Err(_) => SecureBootProbe::Unreadable,
        },
        Err(e) if e.kind() == ErrorKind::NotFound => SecureBootProbe::NotPresent,
        Err(_) => SecureBootProbe::Unreadable,
    };
    classify_secure_boot(probe)
}

#[cfg(test)]
mod tests {
    use super::map_tpm_version;
    use super::{classify_secure_boot, SecureBootProbe};
    use crate::core::system_info::types::SecureBootStatus;
    use crate::core::system_info::types::TpmVersion;
    use windows_sys::Win32::System::TpmBaseServices::{
        TPM_VERSION_12, TPM_VERSION_20, TPM_VERSION_UNKNOWN,
    };

    #[test]
    fn maps_version_1_2() {
        assert_eq!(map_tpm_version(TPM_VERSION_12), TpmVersion::V1_2);
    }

    #[test]
    fn maps_version_2_0() {
        assert_eq!(map_tpm_version(TPM_VERSION_20), TpmVersion::V2_0);
    }

    #[test]
    fn maps_tbs_unknown_constant_to_unknown() {
        assert_eq!(map_tpm_version(TPM_VERSION_UNKNOWN), TpmVersion::Unknown);
    }

    #[test]
    fn maps_out_of_spec_value_to_unknown() {
        // Honesty: any value the ABI doesn't define must degrade to Unknown,
        // never panic and never guess a version.
        assert_eq!(map_tpm_version(7), TpmVersion::Unknown);
    }

    #[test]
    fn secure_boot_value_1_is_enabled() {
        assert_eq!(
            classify_secure_boot(SecureBootProbe::Value(1)),
            Some(SecureBootStatus::Enabled)
        );
    }

    #[test]
    fn secure_boot_value_0_is_disabled() {
        assert_eq!(
            classify_secure_boot(SecureBootProbe::Value(0)),
            Some(SecureBootStatus::Disabled)
        );
    }

    #[test]
    fn secure_boot_non_1_value_is_disabled_not_enabled() {
        // Anything that isn't exactly 1 is honestly "not enabled".
        assert_eq!(
            classify_secure_boot(SecureBootProbe::Value(2)),
            Some(SecureBootStatus::Disabled)
        );
    }

    #[test]
    fn secure_boot_absent_key_is_unsupported_not_disabled() {
        // The crux of the honesty rule: a legacy-BIOS machine has NO Secure Boot
        // to disable. Reporting "Disabled" there would be a lie.
        assert_eq!(
            classify_secure_boot(SecureBootProbe::NotPresent),
            Some(SecureBootStatus::Unsupported)
        );
    }

    #[test]
    fn secure_boot_unreadable_is_none_never_fabricated() {
        // Couldn't read it → report nothing, never a plausible guess.
        assert_eq!(classify_secure_boot(SecureBootProbe::Unreadable), None);
    }

    /// Hardware probe (parity with the NVMe/battery dumps): prints the real
    /// Secure Boot status + CPUID hypervisor bit on THIS machine. Not an
    /// assertion — values are machine-specific. Run with:
    /// `cargo test --lib mbo::tests::dump_security_state -- --ignored --nocapture`
    #[test]
    #[ignore = "hardware probe, prints machine-specific security state"]
    fn dump_security_state() {
        use super::detect_secure_boot;
        use crate::platform::system::windows::cpu::detect_hypervisor_present;
        println!("Secure Boot       : {:?}", detect_secure_boot());
        println!("Hypervisor present: {}", detect_hypervisor_present());
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
