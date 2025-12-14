use crate::core::system_info::types::PowerPlanInfo;
use crate::error::Result;

#[cfg(windows)]
use crate::platform::system_info_windows;

pub fn collect() -> Result<PowerPlanInfo> {
    #[cfg(windows)]
    {
        system_info_windows::get_power_plan_info()
    }

    #[cfg(not(windows))]
    {
        // Power plans are Windows-specific
        Err(crate::error::MscError::other(
            "Power plans are Windows-only",
        ))
    }
}
