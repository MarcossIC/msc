use crate::core::system_info::types::BatteryInfo;
use crate::error::Result;

#[cfg(windows)]
use crate::platform::system::windows::battery::get_battery_info;

pub fn collect() -> Result<BatteryInfo> {
    #[cfg(windows)]
    {
        get_battery_info()
    }

    #[cfg(target_os = "linux")]
    {
        // Could read from /sys/class/power_supply/BAT0/
        Err(crate::error::MscError::other(
            "Not implemented on Linux yet",
        ))
    }

    #[cfg(target_os = "macos")]
    {
        // Could use ioreg or system_profiler
        Err(crate::error::MscError::other(
            "Not implemented on macOS yet",
        ))
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Err(crate::error::MscError::other(
            "Not supported on this platform",
        ))
    }
}
