use crate::core::system_info::memory_prediction;
use crate::core::system_info::types::MemoryInfo;
use crate::error::Result;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

#[cfg(windows)]
use crate::platform::system::windows::mbo::get_motherboard_info;
use crate::platform::system::windows::ram::get_memory_details;

pub fn collect() -> Result<MemoryInfo> {
    let refresh = RefreshKind::nothing()
        .with_memory(MemoryRefreshKind::everything())
        .with_cpu(CpuRefreshKind::everything());
    let sys = System::new_with_specifics(refresh);

    let total = sys.total_memory();
    let available = sys.available_memory();
    let used = sys.used_memory();
    let usage_percent = if total > 0 {
        (used as f32 / total as f32) * 100.0
    } else {
        0.0
    };

    // Get CPU model for memory prediction
    let cpu_model = if let Some(cpu) = sys.cpus().first() {
        cpu.brand().to_string()
    } else {
        "Unknown".to_string()
    };

    // Get detailed info from platform-specific code
    #[cfg(windows)]
    let memory_details = get_memory_details().unwrap_or_default();
    let (ddr_type, speed_mhz, modules, total_slots, used_slots, max_capacity) = (
        memory_details.ddr_type,
        memory_details.speed_mhz,
        memory_details.modules,
        memory_details.total_slots,
        memory_details.used_slots,
        memory_details.max_capacity_bytes,
    );

    #[cfg(not(windows))]
    let (ddr_type, speed_mhz, modules, total_slots, used_slots, max_capacity) =
        (None, None, vec![], None, None, None);

    // Get motherboard model for chipset detection
    #[cfg(windows)]
    let motherboard_model = get_motherboard_info().ok().and_then(|mb| mb.product);

    #[cfg(not(windows))]
    let motherboard_model: Option<String> = None;

    // Perform memory capacity prediction
    let prediction = if total_slots.is_some() || !modules.is_empty() {
        Some(memory_prediction::predict_memory_capacity(
            &cpu_model,
            motherboard_model.as_deref(),
            ddr_type,
            &modules,
            total_slots,
            max_capacity,
        ))
    } else {
        None
    };

    Ok(MemoryInfo {
        total_bytes: total,
        available_bytes: available,
        used_bytes: used,
        usage_percent,
        ddr_type,
        speed_mhz,
        modules,
        total_slots,
        used_slots,
        max_capacity_bytes: max_capacity,
        prediction,
    })
}

pub fn get_fallback() -> MemoryInfo {
    MemoryInfo {
        total_bytes: 0,
        available_bytes: 0,
        used_bytes: 0,
        usage_percent: 0.0,
        ddr_type: None,
        speed_mhz: None,
        modules: vec![],
        total_slots: None,
        used_slots: None,
        max_capacity_bytes: None,
        prediction: None,
    }
}
