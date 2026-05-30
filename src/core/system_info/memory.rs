use crate::core::system_info::memory_prediction;
use crate::core::system_info::types::MemoryInfo;
use crate::error::Result;
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

#[cfg(windows)]
use crate::platform::system::windows::ram::get_memory_details;

/// Collect memory information.
///
/// `cpu_model` and `motherboard_model` are passed in by the caller to avoid
/// re-running CPU/motherboard collection (each is expensive on Windows).
pub fn collect(cpu_model: &str, motherboard_model: Option<&str>) -> Result<MemoryInfo> {
    let refresh = RefreshKind::nothing().with_memory(MemoryRefreshKind::everything());
    let sys = System::new_with_specifics(refresh);

    let total = sys.total_memory();
    let available = sys.available_memory();
    let used = sys.used_memory();
    let usage_percent = if total > 0 {
        (used as f32 / total as f32) * 100.0
    } else {
        0.0
    };

    // Get detailed info from platform-specific code
    #[cfg(windows)]
    let (ddr_type, speed_mhz, modules, total_slots, used_slots, max_capacity) = {
        let memory_details = get_memory_details().unwrap_or_default();
        (
            memory_details.ddr_type,
            memory_details.speed_mhz,
            memory_details.modules,
            memory_details.total_slots,
            memory_details.used_slots,
            memory_details.max_capacity_bytes,
        )
    };

    #[cfg(not(windows))]
    let (ddr_type, speed_mhz, modules, total_slots, used_slots, max_capacity) =
        (None, None, vec![], None, None, None);

    // Perform memory capacity prediction
    let prediction = if total_slots.is_some() || !modules.is_empty() {
        Some(memory_prediction::predict_memory_capacity(
            cpu_model,
            motherboard_model,
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
