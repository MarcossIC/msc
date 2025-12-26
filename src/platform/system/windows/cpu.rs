use crate::error::{MscError, Result};
use serde::Deserialize;
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
}

/// CPU Details returned from Windows-specific queries
pub struct CpuDetailsWindows {
    pub max_frequency_mhz: Option<u64>,
    pub turbo_enabled: Option<bool>,
    pub l2_cache_kb: Option<u32>,
    pub l3_cache_kb: Option<u32>,
    pub numa_nodes: Option<u32>,
}

/// Get CPU additional details using WMI
#[cfg(windows)]
pub fn get_cpu_details() -> Result<CpuDetailsWindows> {
    let wmi_con = WMIConnection::new()
        .map_err(|e| MscError::other(format!("Failed to connect to WMI: {}", e)))?;

    let processors: Vec<Win32Processor> = wmi_con
        .query()
        .map_err(|e| MscError::other(format!("WMI query failed: {}", e)))?;

    let processor = processors.first();

    let max_frequency = processor.and_then(|p| p.max_clock_speed.map(|f| f as u64));
    let turbo_enabled = processor.and_then(|p| p.turbo_mode_enabled);
    let l2_cache_kb = processor.and_then(|p| p.l2_cache_size);
    let l3_cache_kb = processor.and_then(|p| p.l3_cache_size);

    // Get NUMA node count
    let numa_nodes = get_numa_node_count().ok();

    Ok(CpuDetailsWindows {
        max_frequency_mhz: max_frequency,
        turbo_enabled,
        l2_cache_kb,
        l3_cache_kb,
        numa_nodes,
    })
}

#[cfg(not(windows))]
pub fn get_cpu_details() -> Result<CpuDetailsWindows> {
    Err(MscError::other(
        "get_cpu_details sólo está disponible en Windows",
    ))
}

/// Get NUMA node count using Windows API
fn get_numa_node_count() -> Result<u32> {
    use std::process::Command;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-WmiObject -Class Win32_NumaNode | Measure-Object).Count",
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to get NUMA nodes: {}", e)))?;

    let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let count = count_str.parse::<u32>().unwrap_or(1); // Default to 1 if parsing fails

    Ok(count)
}

/// Detect CPU instruction set support using CPUID
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
