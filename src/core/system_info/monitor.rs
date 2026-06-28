use crate::core::system_info::types::MonitorInfo;
use crate::error::Result;

/// Collect EDID-parsed details for every connected display.
///
/// Windows reads each active monitor's EDID driver-less (no admin). Other
/// platforms have no implementation yet, so they return an empty list — the
/// renderer simply omits the section, never a fabricated monitor.
pub fn collect() -> Result<Vec<MonitorInfo>> {
    #[cfg(windows)]
    {
        Ok(crate::platform::system::windows::monitor::get_monitors())
    }

    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

/// Fallback used when the collector thread panics: an empty list.
pub fn get_fallback() -> Vec<MonitorInfo> {
    Vec::new()
}
