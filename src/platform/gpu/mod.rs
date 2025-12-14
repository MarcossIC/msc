//! GPU-specific platform code.
//!
//! Provides GPU metrics collection for different vendors.
//! Currently supports NVIDIA (via NVML), with AMD support planned.

mod nvidia;

pub use nvidia::NvidiaGpuProvider;

use crate::core::system_monitor::GpuProvider;
use crate::error::{MscError, Result};

/// Attempt to get an available GPU provider
///
/// Tries each supported vendor in order of preference.
/// Returns None if no GPU is available.
pub fn get_gpu_provider() -> Result<Box<dyn GpuProvider>> {
    // Try NVIDIA first
    if let Ok(provider) = NvidiaGpuProvider::new() {
        return Ok(Box::new(provider));
    }

    // TODO: Try AMD
    // if let Ok(provider) = AmdGpuProvider::new() {
    //     return Ok(Box::new(provider));
    // }

    Err(MscError::gpu_not_available("No supported GPU found"))
}

/// Check if any GPU is available without initializing provider
pub fn is_gpu_available() -> bool {
    get_gpu_provider().is_ok()
}
