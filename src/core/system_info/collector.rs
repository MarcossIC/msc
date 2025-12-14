use crate::core::system_info::types::*;
use crate::core::system_info::{
    battery, cpu, gpu, memory, motherboard, network, os, power, storage,
};
use crate::error::Result;
use log::warn;

/// Collect all system information
pub fn collect_system_info() -> Result<SystemInfo> {
    let cpu_info = cpu::collect().unwrap_or_else(|e| {
        warn!("Failed to collect CPU info: {}", e);
        cpu::get_fallback()
    });

    let memory_info = memory::collect().unwrap_or_else(|e| {
        warn!("Failed to collect memory info: {}", e);
        eprintln!("Warning: Failed to collect memory info: {}", e);
        memory::get_fallback()
    });

    let gpu_info = gpu::collect().unwrap_or_else(|e| {
        warn!("Failed to collect GPU info: {}", e);
        eprintln!("Warning: Failed to collect GPU info: {}", e);
        vec![]
    });

    let motherboard_info = match motherboard::collect() {
        Ok(mb) => Some(mb),
        Err(e) => {
            warn!("Failed to collect motherboard info: {}", e);
            eprintln!("Warning: Failed to collect motherboard info: {}", e);
            None
        }
    };

    let network_info = network::collect().unwrap_or_else(|e| {
        warn!("Failed to collect network info: {}", e);
        eprintln!("Warning: Failed to collect network info: {}", e);
        network::get_fallback()
    });

    let storage_info = storage::collect().unwrap_or_else(|e| {
        warn!("Failed to collect storage info: {}", e);
        vec![]
    });

    let os_info = os::collect().unwrap_or_else(|e| {
        warn!("Failed to collect OS info: {}", e);
        os::get_fallback()
    });

    // NPU detection is best-effort
    let npu_info = detect_npu();

    // Battery info (only on laptops)
    let battery_info = battery::collect().ok();

    // Power plan info (Windows only)
    let power_plan_info = power::collect().ok();

    Ok(SystemInfo {
        cpu: cpu_info,
        memory: memory_info,
        gpu: gpu_info,
        motherboard: motherboard_info,
        network: network_info,
        storage: storage_info,
        os: os_info,
        npu: npu_info,
        battery: battery_info,
        power_plan: power_plan_info,
    })
}

/// Detect NPU if available (best effort)
fn detect_npu() -> Option<NpuInfo> {
    // Detect NPU from CPU model name
    let cpu_info = cpu::collect().ok()?;
    let model = cpu_info.model.to_lowercase();

    // AMD Ryzen AI processors have NPU
    if model.contains("ryzen ai") {
        // Extract model number to determine NPU TOPS
        let tops = if model.contains("ryzen ai 9") || model.contains("ryzen ai 7") {
            Some(50.0) // Ryzen AI 9 HX 370 and AI 7 350/360 have ~50 TOPS
        } else {
            Some(40.0) // Other Ryzen AI models have ~40 TOPS
        };

        return Some(NpuInfo {
            name: "AMD XDNA NPU".to_string(),
            tops,
        });
    }

    // Intel Core Ultra processors have NPU
    if model.contains("core ultra") || model.contains("meteor lake") || model.contains("arrow lake")
    {
        let tops = if model.contains("ultra 9") || model.contains("ultra 7") {
            Some(34.0) // Intel Core Ultra 7/9 have ~34 TOPS
        } else if model.contains("ultra 5") {
            Some(28.0) // Intel Core Ultra 5 has ~28 TOPS
        } else {
            Some(30.0) // Default
        };

        return Some(NpuInfo {
            name: "Intel AI Boost NPU".to_string(),
            tops,
        });
    }

    None
}
