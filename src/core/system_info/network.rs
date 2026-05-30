use crate::core::system_info::types::NetworkInfo;
use crate::error::Result;
use std::time::Duration;

#[cfg(windows)]
use crate::platform::system::windows::network::{get_network_info, get_network_info_profiled};

pub fn collect() -> Result<NetworkInfo> {
    #[cfg(windows)]
    {
        get_network_info()
    }

    #[cfg(not(windows))]
    {
        // Could use networkmanager on Linux or networksetup on macOS
        Ok(get_fallback())
    }
}

pub fn collect_with_subs() -> Result<(NetworkInfo, Vec<(String, Duration)>)> {
    #[cfg(windows)]
    {
        get_network_info_profiled()
    }

    #[cfg(not(windows))]
    {
        Ok((get_fallback(), Vec::new()))
    }
}

pub fn get_fallback() -> NetworkInfo {
    NetworkInfo {
        wifi_adapters: vec![],
        ethernet_adapters: vec![],
        bluetooth_adapters: vec![],
    }
}
