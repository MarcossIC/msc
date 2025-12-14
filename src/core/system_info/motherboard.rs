use crate::core::system_info::types::MotherboardInfo;
use crate::error::Result;

#[cfg(windows)]
use crate::platform::system_info_windows;

pub fn collect() -> Result<MotherboardInfo> {
    #[cfg(windows)]
    {
        system_info_windows::get_motherboard_info()
    }

    #[cfg(not(windows))]
    {
        // On Linux, could read from /sys/class/dmi/id/
        // On macOS, could use system_profiler
        Err(crate::error::MscError::other(
            "Not implemented on this platform",
        ))
    }
}
