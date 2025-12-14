use crate::core::system_info::types::GpuInfo;
use crate::error::Result;

#[cfg(windows)]
use crate::platform::system_info_windows;

pub fn collect() -> Result<Vec<GpuInfo>> {
    #[cfg(windows)]
    {
        system_info_windows::get_gpu_info()
    }

    #[cfg(not(windows))]
    {
        // On Linux/macOS, could use lspci or system_profiler
        // For now, return empty
        Ok(vec![])
    }
}
