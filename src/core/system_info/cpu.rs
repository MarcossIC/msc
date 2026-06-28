use crate::core::system_info::types::{CpuInfo, CpuInstructionSets};
use crate::error::Result;
use std::time::{Duration, Instant};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

#[cfg(windows)]
use crate::platform::system::windows::cpu::collect_details_profiled;

/// sysinfo needs at least this much elapsed time between the two CPU samples for
/// the usage delta to mean anything.
const USAGE_SAMPLE_INTERVAL: Duration = Duration::from_millis(100);

pub fn collect() -> Result<CpuInfo> {
    collect_with_subs().map(|(info, _)| info)
}

/// Collect CPU info with per-phase sub-timings for the profiler.
///
/// Key timing trick: sysinfo wants ~100ms between its two usage samples. Instead
/// of a DEAD `sleep(100ms)` (paid on every run, on the heaviest collector thread),
/// we take the first sample, run the WMI work in that window, and only top up the
/// leftover — which is usually 0 because the WMI queries already exceed 100ms.
/// That reclaims ~100ms off the hot path while keeping the usage reading.
pub fn collect_with_subs() -> Result<(CpuInfo, Vec<(String, Duration)>)> {
    let mut subs: Vec<(String, Duration)> = Vec::new();

    let refresh = RefreshKind::nothing().with_cpu(CpuRefreshKind::everything());
    let mut sys = System::new_with_specifics(refresh);

    // First usage sample, then start the clock for the inter-sample interval.
    sys.refresh_cpu_all();
    let sample_start = Instant::now();

    let physical_cores = sysinfo::System::physical_core_count().unwrap_or(0);

    let (model, vendor, logical_cores, frequency_mhz) = {
        let cpus = sys.cpus();
        if cpus.is_empty() {
            return Ok((get_fallback(), subs));
        }
        let first = &cpus[0];
        (
            first.brand().to_string(),
            first.vendor_id().to_string(),
            cpus.len(),
            first.frequency(),
        )
    };

    // Platform-specific details run DURING the inter-sample interval.
    #[cfg(windows)]
    let (
        max_frequency_mhz,
        turbo_boost_enabled,
        l2_cache_kb,
        l3_cache_kb,
        numa_nodes,
        instruction_sets,
        amd_topology,
        virtualization,
        temperature_celsius,
        cpu_signature,
        cpu_microcode,
    ) = {
        let (details, temp, isets, amd, virt, sig, mcu, mut wsubs) =
            collect_details_profiled(&model, physical_cores);
        subs.append(&mut wsubs);
        (
            details.as_ref().and_then(|d| d.max_frequency_mhz),
            details.as_ref().and_then(|d| d.turbo_enabled),
            details.as_ref().and_then(|d| d.l2_cache_kb),
            details.as_ref().and_then(|d| d.l3_cache_kb),
            details.as_ref().and_then(|d| d.numa_nodes),
            isets,
            amd,
            virt,
            temp,
            sig,
            mcu,
        )
    };

    #[cfg(not(windows))]
    let (
        max_frequency_mhz,
        turbo_boost_enabled,
        l2_cache_kb,
        l3_cache_kb,
        numa_nodes,
        instruction_sets,
        virtualization,
        temperature_celsius,
    ) = (
        None,
        None,
        None,
        None,
        None,
        CpuInstructionSets::default(),
        None,
        None,
    );

    // Top up to the sample interval ONLY if the WMI work didn't already cover it.
    // The sub-row makes the leftover visible — it should read ~0 in practice.
    let topup = USAGE_SAMPLE_INTERVAL.saturating_sub(sample_start.elapsed());
    if !topup.is_zero() {
        std::thread::sleep(topup);
    }
    subs.push(("cpu.usage_topup_sleep".to_string(), topup));

    // Second sample → usage delta.
    sys.refresh_cpu_all();
    let cpus = sys.cpus();
    let current_usage_percent = if cpus.is_empty() {
        None
    } else {
        let total: f32 = cpus.iter().map(|c| c.cpu_usage()).sum();
        Some(total / cpus.len() as f32)
    };

    let info = CpuInfo {
        model,
        vendor,
        physical_cores,
        logical_cores,
        architecture: std::env::consts::ARCH.to_string(),

        // CPUID processor signature (family/model/stepping). Windows reads it via
        // collect_details_profiled; other platforms don't probe CPUID here.
        #[cfg(windows)]
        cpu_family: cpu_signature.family,
        #[cfg(windows)]
        cpu_model: cpu_signature.model,
        #[cfg(windows)]
        cpu_stepping: cpu_signature.stepping,
        #[cfg(windows)]
        cpu_microcode,
        #[cfg(not(windows))]
        cpu_family: None,
        #[cfg(not(windows))]
        cpu_model: None,
        #[cfg(not(windows))]
        cpu_stepping: None,
        #[cfg(not(windows))]
        cpu_microcode: None,

        frequency_mhz,
        max_frequency_mhz,
        turbo_boost_enabled,

        // Cache
        l1_cache_kb: None, // L1 cache not readily available via WMI
        l2_cache_kb,
        l3_cache_kb,

        // Power and frequency
        tdp_watts: None, // TDP not available via standard WMI
        max_tdp_watts: None,
        turbo_frequency_mhz: max_frequency_mhz, // Same as max frequency for now

        // Instruction sets
        instruction_sets,

        // Virtualization (VT-x/AMD-V firmware + hypervisor presence)
        virtualization,

        // Usage
        current_usage_percent,

        // Thermals
        temperature_celsius,

        // Topology
        numa_nodes,
        #[cfg(windows)]
        chiplet_count: amd_topology.chiplet_count,
        #[cfg(windows)]
        ccd_count: amd_topology.ccd_count,
        #[cfg(windows)]
        ccx_count: amd_topology.ccx_count,
        #[cfg(not(windows))]
        chiplet_count: None,
        #[cfg(not(windows))]
        ccd_count: None,
        #[cfg(not(windows))]
        ccx_count: None,
    };

    Ok((info, subs))
}

pub fn get_fallback() -> CpuInfo {
    CpuInfo {
        model: "Unknown".to_string(),
        vendor: "Unknown".to_string(),
        physical_cores: 0,
        logical_cores: 0,
        architecture: std::env::consts::ARCH.to_string(),
        cpu_family: None,
        cpu_model: None,
        cpu_stepping: None,
        cpu_microcode: None,
        frequency_mhz: 0,
        max_frequency_mhz: None,
        turbo_boost_enabled: None,
        l1_cache_kb: None,
        l2_cache_kb: None,
        l3_cache_kb: None,
        tdp_watts: None,
        max_tdp_watts: None,
        turbo_frequency_mhz: None,
        instruction_sets: CpuInstructionSets::default(),
        virtualization: None,
        current_usage_percent: None,
        temperature_celsius: None,
        numa_nodes: None,
        chiplet_count: None,
        ccd_count: None,
        ccx_count: None,
    }
}
