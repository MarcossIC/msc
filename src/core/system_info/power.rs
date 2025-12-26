use crate::core::system_info::types::PowerPlanInfo;
use crate::error::Result;
#[cfg(windows)]
use crate::platform::system::windows::battery::get_power_plan_info;

pub fn collect() -> Result<PowerPlanInfo> {
    #[cfg(windows)]
    {
        get_power_plan_info()
    }

    #[cfg(not(windows))]
    {
        // Power plans are Windows-specific
        Err(crate::error::MscError::other(
            "Power plans are Windows-only",
        ))
    }
}
