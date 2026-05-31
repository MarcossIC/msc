use crate::core::system_info::types::{AmdGpuMetrics, GpuInfo, NvidiaGpuMetrics};
use crate::error::{MscError, Result};
use serde::Deserialize;
use wmi::WMIConnection;

#[derive(Debug, Default)]
struct GpuTelemetry {
    nvidia_metrics: Option<NvidiaGpuMetrics>,
    amd_metrics: Option<AmdGpuMetrics>,
    vram_bytes: Option<u64>,
    core_clock_mhz: Option<u32>,
    memory_clock_mhz: Option<u32>,
    temperature_celsius: Option<u32>,
    power_draw_watts: Option<f32>,
    fan_speed_percent: Option<u32>,
    memory_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32VideoController {
    name: Option<String>,
    #[serde(rename = "AdapterRAM")]
    adapter_ram: Option<u64>,
    #[serde(rename = "PNPDeviceID")]
    pnp_device_id: Option<String>,
}

fn is_integrated_gpu(name: &str, vendor: &GpuVendor) -> bool {
    match vendor {
        GpuVendor::Intel => !name.contains("Arc"),
        GpuVendor::Amd => name.contains("Radeon") && !name.contains("RX"),
        _ => false,
    }
}

fn detect_vendor(name: &str) -> GpuVendor {
    let name = name.to_ascii_lowercase();

    if name.contains("nvidia") {
        GpuVendor::Nvidia
    } else if name.contains("amd") || name.contains("radeon") {
        GpuVendor::Amd
    } else if name.contains("intel") {
        GpuVendor::Intel
    } else {
        GpuVendor::Unknown
    }
}

/// Extract the PCI **Subsystem Vendor** from a Windows PNP device ID and map it
/// to a board-partner / OEM name.
///
/// The PNP ID looks like `PCI\VEN_10DE&DEV_28E0&SUBSYS_12345678&REV_A1`, where
/// `SUBSYS_` is followed by 8 hex digits: the high 4 are the subsystem *device*
/// id and the **low 4 are the subsystem VENDOR id** — the assembler we want.
/// Returns `None` when there's no `SUBSYS_` token or the subsystem vendor is
/// unset (`0000`). Unknown-but-present ids are surfaced as raw hex rather than a
/// fabricated name (brutal honesty: never invent a brand we can't prove).
fn parse_subsystem_vendor(pnp_id: &str) -> Option<String> {
    let upper = pnp_id.to_ascii_uppercase();
    let after = upper.split("SUBSYS_").nth(1)?;
    let subsys: String = after.chars().take(8).collect();
    if subsys.len() < 8 {
        return None;
    }
    let vendor_id = u16::from_str_radix(&subsys[4..8], 16).ok()?;
    if vendor_id == 0x0000 {
        return None; // No subsystem vendor programmed.
    }
    Some(subsystem_vendor_name(vendor_id))
}

/// Map a PCI subsystem vendor id to a known assembler/OEM. Unknown ids fall back
/// to their raw hex form so we never claim a brand we can't verify.
fn subsystem_vendor_name(id: u16) -> String {
    match id {
        0x1025 => "Acer".to_string(),
        0x1028 => "Dell".to_string(),
        0x1043 => "ASUS".to_string(),
        0x103C => "HP".to_string(),
        0x1414 => "Microsoft".to_string(),
        0x144D => "Samsung".to_string(),
        0x1458 => "Gigabyte".to_string(),
        0x1462 => "MSI".to_string(),
        0x1558 => "Clevo".to_string(),
        0x1849 => "ASRock".to_string(),
        0x17AA => "Lenovo".to_string(),
        0x1A58 => "Razer".to_string(),
        0x1B4C => "Galax / KFA2".to_string(),
        0x1569 => "Palit".to_string(),
        0x196E => "PNY".to_string(),
        0x19DA => "Zotac".to_string(),
        0x3842 => "EVGA".to_string(),
        0x148C => "PowerColor".to_string(),
        0x1DA2 => "Sapphire".to_string(),
        // Reference / Founders designs carry the chip vendor as the subsystem.
        0x10DE => "NVIDIA (reference)".to_string(),
        0x1002 => "AMD (reference)".to_string(),
        0x8086 => "Intel (reference)".to_string(),
        other => format!("0x{other:04X}"),
    }
}

/// Fallback memory type detection for AMD GPUs
///
/// For AMD GPUs, we use a simplified detection since we don't have
/// direct hardware access like NVML for NVIDIA
fn get_amd_gpu_memory_type(model: &str) -> Option<String> {
    // RX 7000 and 6000 Series use GDDR6
    if model.contains("RX 7") || model.contains("RX 6") {
        return Some("GDDR6".to_string());
    }

    None
}

fn collect_nvidia_metrics(index: &mut u32, adapter_ram: Option<u64>) -> GpuTelemetry {
    #[cfg(feature = "nvml")]
    {
        if let Ok(nvidia) = crate::platform::nvidia_nvml::get_nvidia_metrics_nvml(*index) {
            *index += 1;
            return GpuTelemetry {
                vram_bytes: nvidia.memory_total_bytes,
                core_clock_mhz: nvidia.clock_graphics_mhz,
                memory_clock_mhz: nvidia.clock_memory_mhz,
                temperature_celsius: nvidia.temperature_celsius,
                power_draw_watts: nvidia.power_draw_watts,
                fan_speed_percent: nvidia.fan_speed_percent,
                memory_type: crate::platform::nvidia_nvml::get_gpu_memory_type(None),
                nvidia_metrics: Some(nvidia),
                ..Default::default()
            };
        }
    }

    let (core, mem, temp, power, fan) =
        get_nvidia_realtime_metrics_by_index(*index).unwrap_or((None, None, None, None, None));

    let vram = get_nvidia_vram_from_smi_by_index(*index).or(adapter_ram);

    *index += 1;

    GpuTelemetry {
        vram_bytes: vram,
        core_clock_mhz: core,
        memory_clock_mhz: mem,
        temperature_celsius: temp,
        power_draw_watts: power,
        fan_speed_percent: fan,
        ..Default::default()
    }
}

fn collect_amd_metrics(gpu_name: &str, adapter_ram: Option<u64>) -> GpuTelemetry {
    // Métricas específicas AMD (driver / sensores / etc.)
    let amd_metrics = get_amd_metrics(gpu_name);
    // Métricas en tiempo real genéricas
    let (core, mem, temp, power, fan) = get_gpu_realtime_metrics(gpu_name, "AMD");

    GpuTelemetry {
        amd_metrics: Some(amd_metrics),
        vram_bytes: adapter_ram,
        core_clock_mhz: core,
        memory_clock_mhz: mem,
        temperature_celsius: temp,
        power_draw_watts: power,
        fan_speed_percent: fan,
        memory_type: get_amd_gpu_memory_type(gpu_name),
        ..Default::default()
    }
}

/// Get GPU information via direct WMI (Win32_VideoController, root\CIMV2).
///
/// Migrated off PowerShell: spawning `powershell.exe` only to run a
/// `Get-CimInstance` cost 300ms-1s of engine startup on every `sys info`.
/// The `wmi` crate hits the exact same class in microseconds.
pub fn get_gpu_info() -> Result<Vec<GpuInfo>> {
    let wmi =
        WMIConnection::new().map_err(|e| MscError::other(format!("WMI connect failed: {e}")))?;

    let controllers: Vec<Win32VideoController> = wmi
        .raw_query("SELECT Name, AdapterRAM, PNPDeviceID FROM Win32_VideoController")
        .map_err(|e| MscError::other(format!("WMI query failed: {e}")))?;

    let mut gpus = Vec::new();
    let mut nvidia_index = 0u32;

    for gpu in controllers {
        // A row with no name is unusable; skip rather than fail the whole query.
        let name = match gpu.name {
            Some(n) if !n.is_empty() => n,
            _ => continue,
        };

        if name.contains("Basic Display") || name.contains("Microsoft Basic") {
            continue;
        }

        let vendor = detect_vendor(&name);
        let is_integrated = is_integrated_gpu(&name, &vendor);

        // The chip vendor is in `vendor`; the *board assembler* (ASUS, MSI,
        // laptop OEM…) lives in the PCI subsystem vendor inside the PNP ID.
        let subsystem_vendor = gpu
            .pnp_device_id
            .as_deref()
            .and_then(parse_subsystem_vendor);

        let telemetry = match vendor {
            GpuVendor::Nvidia => collect_nvidia_metrics(&mut nvidia_index, gpu.adapter_ram),
            GpuVendor::Amd => collect_amd_metrics(&name, gpu.adapter_ram),
            _ => GpuTelemetry::default(),
        };

        gpus.push(GpuInfo {
            name,
            vendor: format!("{vendor:?}"),
            subsystem_vendor,
            vram_bytes: telemetry.vram_bytes,
            memory_type: telemetry.memory_type,
            is_integrated,
            driver_version: None,
            core_clock_mhz: telemetry.core_clock_mhz,
            memory_clock_mhz: telemetry.memory_clock_mhz,
            temperature_celsius: telemetry.temperature_celsius,
            power_draw_watts: telemetry.power_draw_watts,
            fan_speed_percent: telemetry.fan_speed_percent,
            nvidia_metrics: telemetry.nvidia_metrics,
            amd_metrics: telemetry.amd_metrics,
        });
    }

    Ok(gpus)
}

// Type alias for GPU real-time metrics: (core_clock, mem_clock, temp, power, fan_speed)
type GpuRealtimeMetrics = (
    Option<u32>,
    Option<u32>,
    Option<u32>,
    Option<f32>,
    Option<u32>,
);

// ============================================================================
// DEPRECATED: Old hardcoded NVIDIA metrics function
// ============================================================================
// This function is kept for fallback when NVML is not available, but should
// NOT be used as primary source. NVML provides REAL data from hardware.
// The hardcoded values below are:
// - Not scalable (requires code changes for new GPUs)
// - Prone to errors (values can be wrong or outdated)
// - Imprecise matching (e.g., "5070" matches "5070 Ti", "5070 SUPER")
// - Assumes PCIe generation instead of reading real negotiated speed
//
// Use nvidia_nvml::get_nvidia_metrics_nvml() instead for professional-grade data.
// ============================================================================
// fn get_nvidia_metrics(...) { ... } // REMOVED - See nvidia_nvml.rs

/// Get AMD-specific GPU metrics
fn get_amd_metrics(model: &str) -> crate::core::system_info::types::AmdGpuMetrics {
    use crate::core::system_info::types::AmdGpuMetrics;

    let model_lower = model.to_lowercase();

    // RX 7000 Series (RDNA 3) - 5nm/6nm TSMC
    let (shader_count, compute_units, rop_count, tmu_count, infinity_cache, process_nm) =
        if model_lower.contains("7900 xtx") {
            (
                Some(6144),
                Some(96),
                Some(192),
                Some(384),
                Some(96),
                Some(5),
            )
        } else if model_lower.contains("7900 xt") {
            (
                Some(5376),
                Some(84),
                Some(192),
                Some(336),
                Some(80),
                Some(5),
            )
        } else if model_lower.contains("7900 gre") {
            (
                Some(5120),
                Some(80),
                Some(160),
                Some(320),
                Some(64),
                Some(5),
            )
        } else if model_lower.contains("7800 xt") {
            (Some(3840), Some(60), Some(96), Some(240), Some(64), Some(5))
        } else if model_lower.contains("7700 xt") {
            (Some(3456), Some(54), Some(64), Some(216), Some(48), Some(5))
        // 7600 XT and 7600 have same specs
        } else if model_lower.contains("7600 xt") || model_lower.contains("7600") {
            (Some(2048), Some(32), Some(64), Some(128), Some(32), Some(6))
        }
        // RX 6000 Series (RDNA 2) - 7nm TSMC
        // 6950 XT and 6900 XT have same specs
        else if model_lower.contains("6950 xt") || model_lower.contains("6900 xt") {
            (
                Some(5120),
                Some(80),
                Some(128),
                Some(320),
                Some(128),
                Some(7),
            )
        } else if model_lower.contains("6800 xt") {
            (
                Some(4608),
                Some(72),
                Some(128),
                Some(288),
                Some(128),
                Some(7),
            )
        } else if model_lower.contains("6800") {
            (
                Some(3840),
                Some(60),
                Some(96),
                Some(240),
                Some(128),
                Some(7),
            )
        } else if model_lower.contains("6700 xt") {
            (Some(2560), Some(40), Some(64), Some(160), Some(96), Some(7))
        } else if model_lower.contains("6600 xt") {
            (Some(2048), Some(32), Some(64), Some(128), Some(32), Some(7))
        } else if model_lower.contains("6600") {
            (Some(1792), Some(28), Some(64), Some(112), Some(32), Some(7))
        }
        // Integrated AMD Radeon Graphics (RDNA 2/3)
        else if model_lower.contains("radeon") && model_lower.contains("graphics") {
            // Ryzen AI 300 series (RDNA 3.5)
            if model_lower.contains("890m") {
                (Some(1024), Some(16), Some(32), Some(64), None, Some(4))
            } else if model_lower.contains("880m") {
                (Some(768), Some(12), Some(32), Some(48), None, Some(4))
            }
            // Ryzen 7000/6000 series (RDNA 2) - 780m and 680m have same specs
            else if model_lower.contains("780m") || model_lower.contains("680m") {
                (Some(768), Some(12), Some(32), Some(48), None, Some(6))
            } else {
                (Some(512), Some(8), Some(16), Some(32), None, Some(6))
            }
        } else {
            (None, None, None, None, None, None)
        };

    AmdGpuMetrics {
        shader_count,
        compute_units,
        rop_count,
        tmu_count,
        smartshift_power_watts: None, // Will try to get from real-time metrics
        infinity_cache_mb: infinity_cache,
        process_nm,
    }
}

/// Get real-time GPU metrics (temperature, clocks, fan speed, power draw)
fn get_gpu_realtime_metrics(_model: &str, vendor: &str) -> GpuRealtimeMetrics {
    // Try to get NVIDIA metrics using nvidia-smi
    if vendor.contains("NVIDIA") {
        if let Ok((core, mem, temp, power, fan)) = get_nvidia_realtime_metrics_by_index(0) {
            return (core, mem, temp, power, fan);
        }
    }

    // Try to get AMD metrics using PowerShell or WMI
    if vendor.contains("AMD") {
        if let Ok((core, mem, temp, power, fan)) = get_amd_realtime_metrics() {
            return (core, mem, temp, power, fan);
        }
    }

    // Fallback: no real-time data available
    (None, None, None, None, None)
}

/// Get NVIDIA real-time metrics using nvidia-smi for a specific GPU index
fn get_nvidia_realtime_metrics_by_index(gpu_index: u32) -> Result<GpuRealtimeMetrics> {
    use std::process::Command;

    // Try to run nvidia-smi with specific GPU index
    let output = Command::new("nvidia-smi")
        .args([
            &format!("--id={}", gpu_index),
            "--query-gpu=clocks.current.graphics,clocks.current.memory,temperature.gpu,power.draw,fan.speed",
            "--format=csv,noheader,nounits"
        ])
        .output()
        .map_err(|e| MscError::other(format!("Failed to run nvidia-smi: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Parse CSV output: core_clock, mem_clock, temp, power, fan
    let parts: Vec<&str> = output_str.trim().split(',').collect();

    if parts.len() >= 5 {
        let core_clock = parts[0].trim().parse::<u32>().ok();
        let mem_clock = parts[1].trim().parse::<u32>().ok();
        let temp = parts[2].trim().parse::<u32>().ok();
        let power = parts[3].trim().parse::<f32>().ok();
        let fan = parts[4].trim().parse::<u32>().ok();

        return Ok((core_clock, mem_clock, temp, power, fan));
    }

    Ok((None, None, None, None, None))
}

/// Get NVIDIA VRAM size using nvidia-smi for a specific GPU index (fallback when NVML fails)
///
/// This function queries nvidia-smi for the total memory size, which is more
/// reliable than Win32_VideoController.AdapterRAM (which has a 4GB limit).
fn get_nvidia_vram_from_smi_by_index(gpu_index: u32) -> Option<u64> {
    use std::process::Command;

    // Query total memory in MiB for specific GPU
    let output = Command::new("nvidia-smi")
        .args([
            &format!("--id={}", gpu_index),
            "--query-gpu=memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mem_mib = output_str.trim().parse::<u64>().ok()?;

    // Convert MiB to bytes
    Some(mem_mib * 1024 * 1024)
}

/// Get AMD real-time metrics using PowerShell
fn get_amd_realtime_metrics() -> Result<GpuRealtimeMetrics> {
    // AMD metrics are harder to get without ADLX/ADL SDK
    // For now, return None - could be extended with WMI queries or third-party tools
    Ok((None, None, None, None, None))
}
