use crate::error::{MscError, Result};
use serde::Deserialize;
#[cfg(windows)]
use crate::core::system_info::types::{CpuInstructionSets, VirtualizationInfo};
#[cfg(windows)]
use std::time::{Duration, Instant};
#[cfg(windows)]
use wmi::WMIConnection;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Win32Processor {
    max_clock_speed: Option<u32>,
    turbo_mode_enabled: Option<bool>,
    l2_cache_size: Option<u32>, // KB
    l3_cache_size: Option<u32>, // KB
    _number_of_cores: Option<u32>,
    _number_of_logical_processors: Option<u32>,
    // VT-x/AMD-V enabled in firmware. NULL on some systems → Option.
    virtualization_firmware_enabled: Option<bool>,
}

/// CPU Details returned from Windows-specific queries
pub struct CpuDetailsWindows {
    pub max_frequency_mhz: Option<u64>,
    pub turbo_enabled: Option<bool>,
    pub l2_cache_kb: Option<u32>,
    pub l3_cache_kb: Option<u32>,
    pub numa_nodes: Option<u32>,
    /// `Win32_Processor.VirtualizationFirmwareEnabled` — VT-x/AMD-V on in BIOS.
    pub virtualization_firmware_enabled: Option<bool>,
}

/// Get CPU additional details using a CALLER-OWNED WMI connection.
///
/// Takes `&WMIConnection` (instead of opening its own) so the processor query and
/// the NUMA count reuse the same `root\cimv2` handle as the rest of the CPU
/// collection — one COM/WMI handshake instead of several.
#[cfg(windows)]
pub fn get_cpu_details(wmi_con: &WMIConnection) -> Result<CpuDetailsWindows> {
    let processors: Vec<Win32Processor> = wmi_con
        .query()
        .map_err(|e| MscError::other(format!("WMI query failed: {}", e)))?;

    let processor = processors.first();

    let max_frequency = processor.and_then(|p| p.max_clock_speed.map(|f| f as u64));
    let turbo_enabled = processor.and_then(|p| p.turbo_mode_enabled);
    let l2_cache_kb = processor.and_then(|p| p.l2_cache_size);
    let l3_cache_kb = processor.and_then(|p| p.l3_cache_size);
    let virtualization_firmware_enabled = processor.and_then(|p| p.virtualization_firmware_enabled);

    // NUMA node count — reuses the same connection.
    let numa_nodes = get_numa_node_count(wmi_con).ok();

    Ok(CpuDetailsWindows {
        max_frequency_mhz: max_frequency,
        turbo_enabled,
        l2_cache_kb,
        l3_cache_kb,
        numa_nodes,
        virtualization_firmware_enabled,
    })
}

/// Read the loaded microcode (patch) revision from the Windows registry —
/// driver-less and without admin.
///
/// Windows publishes the active microcode under
/// `HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0` as the DWORD
/// `Current Record Version` (e.g. `0xB600032`). This is the same value tools like
/// HWiNFO show as "MCU" — but they read it via a ring-0 driver (MSR `0x8B`); the
/// registry exposes it to a normal user. Returns `None` if the key/value is absent.
#[cfg(windows)]
fn read_microcode_revision() -> Option<u32> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let value: u32 = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0")
        .ok()?
        .get_value("Current Record Version")
        .ok()?;

    // 0 means "no microcode update loaded" — report absence rather than a bogus 0x0.
    (value != 0).then_some(value)
}

#[cfg(not(windows))]
pub fn get_cpu_details() -> Result<CpuDetailsWindows> {
    Err(MscError::other(
        "get_cpu_details sólo está disponible en Windows",
    ))
}

/// Get NUMA node count over a caller-owned WMI connection (no extra handshake).
#[cfg(windows)]
fn get_numa_node_count(wmi_con: &WMIConnection) -> Result<u32> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32NumaNode {
        #[serde(default, rename = "Name")]
        _name: Option<String>,
    }

    let nodes: Vec<Win32NumaNode> = wmi_con
        .query()
        .map_err(|e| MscError::other(format!("WMI query failed: {}", e)))?;
    Ok(nodes.len().max(1) as u32)
}

/// Collect all Windows-specific CPU enrichment over ONE shared WMI connection,
/// with per-phase sub-timings for the profiler.
///
/// Before this, the processor query, the NUMA count, and the thermal perf counter
/// each opened their OWN `WMIConnection` — 3 separate COM/WMI handshakes for data
/// that all lives in `root\cimv2`. Now we open it once and reuse it.
///
/// The returned subs (`cpu.wmi_details`, `cpu.temp`) make each phase visible in
/// the profile. This matters: the thermal perf-counter class
/// (`Win32_PerfFormattedData_*`) triggers a multi-second cold-start the FIRST time
/// any process queries a performance counter in a Windows session — `cpu.temp` is
/// the row that exposes that instead of letting it inflate `cpu` invisibly.
#[cfg(windows)]
pub fn collect_details_profiled(
    cpu_brand: &str,
    physical_cores: usize,
) -> (
    Option<CpuDetailsWindows>,
    Option<u32>,
    CpuInstructionSets,
    AmdTopology,
    Option<VirtualizationInfo>,
    CpuSignature,
    Option<u32>,
    Vec<(String, Duration)>,
) {
    let mut subs = Vec::with_capacity(2);

    // Pure CPUID / pattern match — microseconds, no WMI, not worth a sub-row.
    let instruction_sets = detect_cpu_instruction_sets();
    let amd_topology = detect_amd_topology(cpu_brand, physical_cores);
    // CPUID hypervisor-present bit — pure, sub-microsecond.
    let hypervisor_present = detect_hypervisor_present();
    // CPUID processor signature (family/model/stepping) — pure, sub-microsecond.
    let signature = detect_cpu_signature();
    // Microcode revision — REGISTRY read, deliberately INDEPENDENT of the WMI
    // `Win32_Processor` query below: that query errors on some OEM laptops, and we
    // must not let a flaky WMI call drag down a driver-less registry value.
    let microcode = read_microcode_revision();

    // One `root\cimv2` connection shared by every query below.
    let con = WMIConnection::new().ok();

    let t = Instant::now();
    let details = con.as_ref().and_then(|c| get_cpu_details(c).ok());
    subs.push(("cpu.wmi_details".to_string(), t.elapsed()));

    let t = Instant::now();
    let temperature = con.as_ref().and_then(get_cpu_temperature);
    subs.push(("cpu.temp".to_string(), t.elapsed()));

    // Combine the firmware flag (WMI) with the hypervisor bit (CPUID). Always
    // Some on Windows: the CPUID bit is always readable even when WMI omits the
    // firmware flag.
    let virtualization = Some(VirtualizationInfo {
        firmware_enabled: details.as_ref().and_then(|d| d.virtualization_firmware_enabled),
        hypervisor_present,
    });

    (
        details,
        temperature,
        instruction_sets,
        amd_topology,
        virtualization,
        signature,
        microcode,
        subs,
    )
}

/// Read the CPUID "hypervisor present" bit (leaf 1, ECX bit 31).
///
/// `true` means a hypervisor is active on this machine — but NOT necessarily a
/// VM: a Windows 11 host with Hyper-V, WSL2, or VBS/Memory Integrity enabled runs
/// the OS itself atop the hypervisor and sets the bit too. The caller labels it
/// honestly as "active", never "is a VM". Pure CPUID, no WMI, no admin.
///
/// Gated by **architecture, not OS**: CPUID is an x86-only instruction, so on
/// Windows-on-ARM (Snapdragon) this whole module still compiles but there is no
/// `cpuid` to call — the non-x86 stub below answers instead.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn detect_hypervisor_present() -> bool {
    use raw_cpuid::CpuId;

    CpuId::new()
        .get_feature_info()
        .map(|f| f.has_hypervisor())
        .unwrap_or(false)
}

/// Non-x86 stub (e.g. Windows-on-ARM): no CPUID, so no hypervisor bit to read.
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn detect_hypervisor_present() -> bool {
    false
}

/// Raw CPUID processor signature (leaf 1): the COMPUTED family, model and stepping.
///
/// `raw_cpuid` already folds the base + extended fields the way AMD/Intel define
/// them, so an AMD Zen 5 part reports family `0x1A`, model `0x60` — exactly the hex
/// values tools like MSI Afterburner print. Pure CPUID: sub-microsecond, no WMI, no
/// admin. All-`None` only if CPUID leaf 1 is unavailable.
///
/// x86-only by architecture (see [`detect_hypervisor_present`]); the non-x86 stub
/// returns all-`None`.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn detect_cpu_signature() -> CpuSignature {
    use raw_cpuid::CpuId;

    CpuId::new()
        .get_feature_info()
        .map(|f| CpuSignature {
            family: Some(f.family_id()),
            model: Some(f.model_id()),
            stepping: Some(f.stepping_id()),
        })
        .unwrap_or(CpuSignature {
            family: None,
            model: None,
            stepping: None,
        })
}

/// Non-x86 stub (e.g. Windows-on-ARM): no CPUID → no family/model/stepping.
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn detect_cpu_signature() -> CpuSignature {
    CpuSignature {
        family: None,
        model: None,
        stepping: None,
    }
}

/// Detect CPU instruction set support using CPUID.
///
/// x86-only by architecture (see [`detect_hypervisor_present`]); the x86 SIMD flags
/// (AVX, SSE, AES-NI…) don't exist on ARM, so the non-x86 stub returns an empty set.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
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

/// Non-x86 stub (e.g. Windows-on-ARM): the x86 instruction-set flags don't apply.
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn detect_cpu_instruction_sets() -> crate::core::system_info::types::CpuInstructionSets {
    crate::core::system_info::types::CpuInstructionSets::default()
}

/// Convert a `MSAcpi_ThermalZoneTemperature.CurrentTemperature` reading (tenths
/// of a Kelvin) to whole degrees Celsius, rejecting implausible values.
///
/// Firmware that doesn't truly support the sensor commonly reports the absolute
/// placeholder `2732` (≈0 °C / 273.2 K) or wild values, so we keep only readings
/// inside a sane CPU range (1–120 °C) and discard everything else as "no data".
fn tenths_kelvin_to_celsius(tenths_kelvin: u32) -> Option<u32> {
    // °C = K - 273.15; here K = tenths_kelvin / 10.
    let celsius = (tenths_kelvin as f64 / 10.0) - 273.15;
    if (1.0..=120.0).contains(&celsius) {
        Some(celsius.round() as u32)
    } else {
        None
    }
}

/// Read the CPU/SoC temperature from the ACPI thermal zone, driver-less.
///
/// There is no driver-less way to read the exact AMD Tctl / Intel DTS sensor, so
/// we use the ACPI thermal zone, which on laptops tracks the SoC closely. When
/// several zones exist we take the **hottest** plausible reading (the active
/// thermal concern).
///
/// Two providers expose the same zones, tried in order:
///   1. `Win32_PerfFormattedData_Counters_ThermalZoneInformation` (`root\cimv2`)
///      — readable by a NORMAL user. `HighPrecisionTemperature` is tenths of a
///      Kelvin, `Temperature` is whole Kelvin. This is the one that actually
///      returns data on locked-down machines.
///   2. `MSAcpi_ThermalZoneTemperature` (`root\wmi`) — same data, but the class
///      returns ACCESS DENIED unless `sys info` runs elevated. Only useful as a
///      fallback when the perf counter is empty AND we happen to be elevated.
///
/// Returns `None` only when neither provider yields a usable reading.
#[cfg(windows)]
pub fn get_cpu_temperature(wmi: &WMIConnection) -> Option<u32> {
    // Primary: thermal-zone perf counter (no elevation required), on the shared
    // `root\cimv2` connection passed in by the caller.
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct ThermalZonePerf {
        temperature: Option<u32>,                // whole Kelvin
        high_precision_temperature: Option<u32>, // tenths of a Kelvin
    }

    let zones: Vec<ThermalZonePerf> = wmi
        .raw_query(
            "SELECT Temperature, HighPrecisionTemperature \
             FROM Win32_PerfFormattedData_Counters_ThermalZoneInformation",
        )
        .unwrap_or_default();

    let hottest = zones
        .into_iter()
        .filter_map(|z| {
            // Prefer the tenths-of-Kelvin field; fall back to whole Kelvin
            // (scaled to tenths) so the same converter/range check applies.
            z.high_precision_temperature
                .filter(|&v| v > 0)
                .or_else(|| z.temperature.map(|k| k.saturating_mul(10)))
                .and_then(tenths_kelvin_to_celsius)
        })
        .max();

    if hottest.is_some() {
        return hottest;
    }

    // Fallback: MSAcpi_ThermalZoneTemperature on root\wmi (elevated only). This is
    // a DIFFERENT namespace, so it gets its own connection — but only when the
    // perf counter above came back empty.
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct MSAcpiThermalZoneTemperature {
        current_temperature: Option<u32>, // tenths of a Kelvin
    }

    let wmi_acpi = WMIConnection::with_namespace_path("root\\wmi").ok()?;
    let zones: Vec<MSAcpiThermalZoneTemperature> = wmi_acpi
        .raw_query("SELECT CurrentTemperature FROM MSAcpi_ThermalZoneTemperature")
        .unwrap_or_default();

    zones
        .into_iter()
        .filter_map(|z| z.current_temperature)
        .filter_map(tenths_kelvin_to_celsius)
        .max()
}

#[cfg(not(windows))]
pub fn get_cpu_temperature() -> Option<u32> {
    None
}

/// Raw CPUID processor signature (computed family/model/stepping). See
/// [`detect_cpu_signature`].
pub struct CpuSignature {
    pub family: Option<u8>,
    pub model: Option<u8>,
    pub stepping: Option<u8>,
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

#[cfg(test)]
mod tests {
    use super::tenths_kelvin_to_celsius;

    #[test]
    fn converts_plausible_readings() {
        // 3132 tenths-K = 313.2 K = 40.05 °C -> 40
        assert_eq!(tenths_kelvin_to_celsius(3132), Some(40));
        // 3431 tenths-K = 343.1 K = 69.95 °C -> 70
        assert_eq!(tenths_kelvin_to_celsius(3431), Some(70));
        // 2982 tenths-K = 298.2 K = 25.05 °C -> 25
        assert_eq!(tenths_kelvin_to_celsius(2982), Some(25));
    }

    #[test]
    fn rejects_zero_celsius_placeholder() {
        // 2732 tenths-K ≈ 0.05 °C — the classic "unsupported" sentinel.
        assert_eq!(tenths_kelvin_to_celsius(2732), None);
    }

    #[test]
    fn rejects_out_of_range_values() {
        assert_eq!(tenths_kelvin_to_celsius(0), None); // absurd low
        assert_eq!(tenths_kelvin_to_celsius(5000), None); // 226.85 °C — absurd high
    }
}
