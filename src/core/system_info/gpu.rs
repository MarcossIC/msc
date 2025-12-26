use crate::core::system_info::types::GpuInfo;
use crate::error::Result;

#[cfg(windows)]
use crate::platform::system::windows::gpu::get_gpu_info;

pub fn collect() -> Result<Vec<GpuInfo>> {
    #[cfg(windows)]
    {
        get_gpu_info()
    }

    #[cfg(not(windows))]
    {
        // On Linux/macOS, could use lspci or system_profiler
        // For now, return empty
        Ok(vec![])
    }
}
