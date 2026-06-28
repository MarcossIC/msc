//! Windows OS details that `sysinfo` doesn't expose honestly.
//!
//! The kernel's own version strings are a trap: `HKLM\...\CurrentVersion`
//! publishes `ProductName = "Windows 10 ..."` and `CurrentMajorVersionNumber = 10`
//! even on Windows 11 — Microsoft NEVER updated them for the 10→11 jump. So the
//! generation ("10" vs "11") is DERIVED from the build number, which is the only
//! reliable signal (Microsoft's documented threshold: build >= 22000 = Windows 11).
//!
//! Architecture mirrors `mbo.rs`: every mapping is a PURE function (golden-tested
//! below, no registry/WMI), and the registry/WMI/env I/O is isolated in the
//! `detect_*`/`read_*` fetchers. Driver-less, no admin.

use crate::core::system_info::types::HvciStatus;

/// Enriched OS info sourced from the Windows registry + DeviceGuard WMI.
/// `name` is `None` when the generation can't be determined honestly (the caller
/// then falls back to the generic `sysinfo` name rather than guessing).
pub struct WindowsOsInfo {
    pub name: Option<String>,
    pub edition: Option<String>,
    pub display_version: Option<String>,
    pub build: Option<String>,
    pub hvci: Option<HvciStatus>,
    pub current_user: Option<String>,
}

/// Honest Windows generation ("10"/"11") from the kernel version numbers.
///
/// `ProductName` and `CurrentMajorVersionNumber` were never updated for the 10→11
/// jump (both still report 10 on Windows 11), so the ONLY reliable signal is the
/// build number: Microsoft's documented threshold is build >= 22000 = Windows 11.
/// Returns `None` for anything we can't map with confidence — we refuse to guess.
/// Pure (no I/O) — covered by golden tests.
fn windows_generation(major: u32, build: u32) -> Option<&'static str> {
    match major {
        10 if build >= 22000 => Some("11"),
        10 => Some("10"),
        _ => None,
    }
}

/// Compose the honest product name from the derived generation and edition word.
/// Requires a generation (the trustworthy part); without it we return `None` so
/// the caller keeps the generic `sysinfo` name instead of inventing one. Pure.
fn windows_product_name(generation: Option<&str>, edition_word: Option<&str>) -> Option<String> {
    let g = generation?;
    Some(match edition_word {
        Some(e) => format!("Windows {g} {e}"),
        None => format!("Windows {g}"),
    })
}

/// Map a registry `EditionID` to its friendly marketing name. Returns `None` for
/// editions we don't have a verified mapping for — the caller then shows the raw
/// `EditionID` rather than a made-up label. Pure — golden-tested.
fn map_edition_id(id: &str) -> Option<&'static str> {
    match id {
        "Core" | "CoreN" => Some("Home"),
        "CoreSingleLanguage" => Some("Home Single Language"),
        "CoreCountrySpecific" => Some("Home China"),
        "Professional" | "ProfessionalN" => Some("Pro"),
        "ProfessionalEducation" | "ProfessionalEducationN" => Some("Pro Education"),
        "ProfessionalWorkstation" | "ProfessionalWorkstationN" => Some("Pro for Workstations"),
        "Enterprise" | "EnterpriseN" => Some("Enterprise"),
        "EnterpriseS" | "EnterpriseSN" => Some("Enterprise LTSC"),
        "Education" | "EducationN" => Some("Education"),
        "IoTEnterprise" => Some("IoT Enterprise"),
        "ServerStandard" => Some("Server Standard"),
        "ServerDatacenter" => Some("Server Datacenter"),
        _ => None,
    }
}

/// Build the "Home (Core)" edition display string from a raw `EditionID`.
/// Falls back to the raw id alone when we have no friendly mapping. Pure.
fn edition_display(edition_id: &str) -> String {
    match map_edition_id(edition_id) {
        Some(word) => format!("{word} ({edition_id})"),
        None => edition_id.to_string(),
    }
}

/// Compose the full build string: `CurrentBuild` + `.UBR` (Update Build Revision),
/// e.g. `"26200" + 8655 -> "26200.8655"`. Empty/blank base → `None`. Pure.
fn format_build(current_build: &str, ubr: Option<u32>) -> Option<String> {
    let cb = current_build.trim();
    if cb.is_empty() {
        return None;
    }
    Some(match ubr {
        Some(u) => format!("{cb}.{u}"),
        None => cb.to_string(),
    })
}

/// Pure mapping from the two `Win32_DeviceGuard` flags to a reported status.
///
/// `running` wins: if HVCI is actually running it's `Running` regardless of how
/// the policy reads. Configured-but-not-running is kept DISTINCT from `Off` so we
/// never claim "Enabled" for something merely scheduled. No I/O — golden-tested.
pub fn classify_hvci(configured: bool, running: bool) -> HvciStatus {
    match (configured, running) {
        (_, true) => HvciStatus::Running,
        (true, false) => HvciStatus::ConfiguredNotRunning,
        (false, false) => HvciStatus::Off,
    }
}

/// Raw registry values from `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`.
#[cfg(windows)]
struct WindowsVersionRaw {
    edition_id: Option<String>,
    display_version: Option<String>,
    current_build: Option<String>,
    ubr: Option<u32>,
    major: Option<u32>,
}

/// Read the Windows version block from the registry — no admin, no WMI.
/// I/O is isolated here so the mappings above stay pure and unit-testable.
#[cfg(windows)]
fn read_windows_version_registry() -> WindowsVersionRaw {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let empty = || WindowsVersionRaw {
        edition_id: None,
        display_version: None,
        current_build: None,
        ubr: None,
        major: None,
    };

    let key = match RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
    {
        Ok(k) => k,
        Err(_) => return empty(),
    };

    // Trim and drop empty strings so a present-but-blank value reads as absent.
    let get_str = |name: &str| -> Option<String> {
        key.get_value::<String, _>(name)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let get_dword = |name: &str| -> Option<u32> { key.get_value::<u32, _>(name).ok() };

    WindowsVersionRaw {
        edition_id: get_str("EditionID"),
        display_version: get_str("DisplayVersion"),
        current_build: get_str("CurrentBuild"),
        ubr: get_dword("UBR"),
        major: get_dword("CurrentMajorVersionNumber"),
    }
}

/// HVCI service id inside the `Win32_DeviceGuard` security-services arrays.
#[cfg(windows)]
const DEVICE_GUARD_HVCI: u32 = 2;

/// Detect the real HVCI state via `Win32_DeviceGuard` (DeviceGuard WMI namespace).
///
/// Reads BOTH `SecurityServicesConfigured` and `SecurityServicesRunning` so we
/// can tell "configured by policy" from "actually running now". Returns `None`
/// when the class/namespace is unavailable (older Windows) — undetermined, never
/// a fabricated "Off". WMI I/O isolated here; [`classify_hvci`] stays pure.
#[cfg(windows)]
pub fn detect_hvci() -> Option<HvciStatus> {
    use serde::Deserialize;
    use wmi::WMIConnection;

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32DeviceGuard {
        security_services_configured: Option<Vec<u32>>,
        security_services_running: Option<Vec<u32>>,
    }

    let wmi = WMIConnection::with_namespace_path("root\\Microsoft\\Windows\\DeviceGuard").ok()?;
    let rows: Vec<Win32DeviceGuard> = wmi
        .raw_query(
            "SELECT SecurityServicesConfigured, SecurityServicesRunning FROM Win32_DeviceGuard",
        )
        .ok()?;
    let row = rows.into_iter().next()?;

    let has_hvci = |arr: &Option<Vec<u32>>| {
        arr.as_deref()
            .map(|v| v.contains(&DEVICE_GUARD_HVCI))
            .unwrap_or(false)
    };
    Some(classify_hvci(
        has_hvci(&row.security_services_configured),
        has_hvci(&row.security_services_running),
    ))
}

/// The interactive user the process runs as, formatted like `whoami`:
/// `DOMAIN\user` (or just `user` when no domain is set). Driver-less, no admin —
/// reads the same `USERDOMAIN`/`USERNAME` the shell exposes.
#[cfg(windows)]
pub fn current_user() -> Option<String> {
    let user = std::env::var("USERNAME")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let domain = std::env::var("USERDOMAIN")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Some(match domain {
        Some(d) => format!("{d}\\{user}"),
        None => user,
    })
}

/// Assemble all Windows-specific OS details (registry + DeviceGuard WMI + env).
#[cfg(windows)]
pub fn get_windows_os_info() -> WindowsOsInfo {
    let raw = read_windows_version_registry();

    let build_num = raw
        .current_build
        .as_deref()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let major = raw.major.unwrap_or(0);

    let generation = windows_generation(major, build_num);
    let edition_word = raw.edition_id.as_deref().and_then(map_edition_id);

    WindowsOsInfo {
        name: windows_product_name(generation, edition_word),
        edition: raw.edition_id.as_deref().map(edition_display),
        display_version: raw.display_version,
        build: raw
            .current_build
            .as_deref()
            .and_then(|cb| format_build(cb, raw.ubr)),
        hvci: detect_hvci(),
        current_user: current_user(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- windows_generation: the core honesty rule ---

    #[test]
    fn generation_build_at_or_above_22000_is_eleven() {
        // The crux: the registry says major=10, but build >= 22000 is Windows 11.
        assert_eq!(windows_generation(10, 22000), Some("11"));
        assert_eq!(windows_generation(10, 26200), Some("11"));
    }

    #[test]
    fn generation_build_below_22000_is_ten() {
        assert_eq!(windows_generation(10, 19045), Some("10"));
        assert_eq!(windows_generation(10, 21999), Some("10"));
    }

    #[test]
    fn generation_unknown_major_is_none_never_guessed() {
        // We only claim what we can map with confidence.
        assert_eq!(windows_generation(6, 9600), None); // Win 8.1-era major
        assert_eq!(windows_generation(0, 0), None);
    }

    // --- windows_product_name ---

    #[test]
    fn product_name_combines_generation_and_edition() {
        assert_eq!(
            windows_product_name(Some("11"), Some("Home")),
            Some("Windows 11 Home".to_string())
        );
    }

    #[test]
    fn product_name_without_edition_is_just_generation() {
        assert_eq!(
            windows_product_name(Some("11"), None),
            Some("Windows 11".to_string())
        );
    }

    #[test]
    fn product_name_without_generation_is_none_to_fall_back() {
        // No trustworthy generation → caller keeps the sysinfo name, we don't
        // invent "Windows Home".
        assert_eq!(windows_product_name(None, Some("Home")), None);
        assert_eq!(windows_product_name(None, None), None);
    }

    // --- map_edition_id ---

    #[test]
    fn edition_core_maps_to_home() {
        assert_eq!(map_edition_id("Core"), Some("Home"));
    }

    #[test]
    fn edition_professional_maps_to_pro() {
        assert_eq!(map_edition_id("Professional"), Some("Pro"));
    }

    #[test]
    fn edition_unknown_id_is_none() {
        assert_eq!(map_edition_id("SomeFutureSku"), None);
    }

    #[test]
    fn edition_display_keeps_raw_id_in_parens() {
        assert_eq!(edition_display("Core"), "Home (Core)");
    }

    #[test]
    fn edition_display_falls_back_to_raw_id_when_unmapped() {
        assert_eq!(edition_display("SomeFutureSku"), "SomeFutureSku");
    }

    // --- format_build ---

    #[test]
    fn build_appends_ubr() {
        assert_eq!(
            format_build("26200", Some(8655)),
            Some("26200.8655".to_string())
        );
    }

    #[test]
    fn build_without_ubr_is_base_only() {
        assert_eq!(format_build("26200", None), Some("26200".to_string()));
    }

    #[test]
    fn build_blank_base_is_none() {
        assert_eq!(format_build("", Some(8655)), None);
        assert_eq!(format_build("   ", None), None);
    }

    // --- classify_hvci ---

    #[test]
    fn hvci_running_is_running_regardless_of_configured() {
        assert_eq!(classify_hvci(true, true), HvciStatus::Running);
        assert_eq!(classify_hvci(false, true), HvciStatus::Running);
    }

    #[test]
    fn hvci_configured_but_not_running_is_distinct_from_off() {
        // Honesty: configured-but-not-running must NOT read as "Enabled".
        assert_eq!(
            classify_hvci(true, false),
            HvciStatus::ConfiguredNotRunning
        );
    }

    #[test]
    fn hvci_neither_is_off() {
        assert_eq!(classify_hvci(false, false), HvciStatus::Off);
    }

    // Hardware probe (parity with mbo's dump_security_state): prints the real
    // OS details on THIS machine. Not an assertion — values are machine-specific.
    // `cargo test --lib platform::system::windows::os::tests::dump_os_state -- --ignored --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore = "hardware probe, prints machine-specific OS state"]
    fn dump_os_state() {
        let info = get_windows_os_info();
        println!("name           : {:?}", info.name);
        println!("edition        : {:?}", info.edition);
        println!("display_version: {:?}", info.display_version);
        println!("build          : {:?}", info.build);
        println!("hvci           : {:?}", info.hvci);
        println!("current_user   : {:?}", info.current_user);
    }
}
