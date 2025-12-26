use crate::core::system_info::types::{CpuInfo, CpuInstructionSets};
use crate::error::Result;
use sysinfo::{CpuRefreshKind, RefreshKind, System};

#[cfg(windows)]
use crate::platform::system::windows::cpu::{
    detect_amd_topology, detect_cpu_instruction_sets, get_cpu_details,
};

pub fn collect() -> Result<CpuInfo> {
    let refresh = RefreshKind::nothing().with_cpu(CpuRefreshKind::everything());
    let mut sys = System::new_with_specifics(refresh);

    // Need to refresh twice to get accurate frequency and usage
    sys.refresh_cpu_all();
    std::thread::sleep(std::time::Duration::from_millis(100));
    sys.refresh_cpu_all();

    let cpus = sys.cpus();
    if cpus.is_empty() {
        return Ok(get_fallback());
    }

    let first_cpu = &cpus[0];
    let physical_cores = sysinfo::System::physical_core_count().unwrap_or(0);

    // Calculate current CPU usage
    let total_usage: f32 = cpus.iter().map(|cpu| cpu.cpu_usage()).sum();
    let current_usage_percent = if !cpus.is_empty() {
        Some(total_usage / cpus.len() as f32)
    } else {
        None
    };

    // Get platform-specific details
    #[cfg(windows)]
    let (
        max_frequency_mhz,
        turbo_boost_enabled,
        l2_cache_kb,
        l3_cache_kb,
        numa_nodes,
        instruction_sets,
        amd_topology,
    ) = {
        let details = get_cpu_details().ok();
        let instruction_sets = detect_cpu_instruction_sets();
        let amd_topology = detect_amd_topology(first_cpu.brand(), physical_cores);

        (
            details.as_ref().and_then(|d| d.max_frequency_mhz),
            details.as_ref().and_then(|d| d.turbo_enabled),
            details.as_ref().and_then(|d| d.l2_cache_kb),
            details.as_ref().and_then(|d| d.l3_cache_kb),
            details.as_ref().and_then(|d| d.numa_nodes),
            instruction_sets,
            amd_topology,
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
    ) = (None, None, None, None, None, CpuInstructionSets::default());

    Ok(CpuInfo {
        model: first_cpu.brand().to_string(),
        vendor: first_cpu.vendor_id().to_string(),
        physical_cores,
        logical_cores: cpus.len(),
        architecture: std::env::consts::ARCH.to_string(),
        frequency_mhz: first_cpu.frequency(),
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

        // Usage
        current_usage_percent,

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
    })
}

pub fn get_fallback() -> CpuInfo {
    CpuInfo {
        model: "Unknown".to_string(),
        vendor: "Unknown".to_string(),
        physical_cores: 0,
        logical_cores: 0,
        architecture: std::env::consts::ARCH.to_string(),
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
        current_usage_percent: None,
        numa_nodes: None,
        chiplet_count: None,
        ccd_count: None,
        ccx_count: None,
    }
}
