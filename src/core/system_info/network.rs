use crate::core::system_info::types::NetworkInfo;
use crate::error::Result;

#[cfg(windows)]
use crate::platform::system_info_windows;

pub fn collect() -> Result<NetworkInfo> {
    #[cfg(windows)]
    {
        system_info_windows::get_network_info()
    }

    #[cfg(not(windows))]
    {
        // Could use networkmanager on Linux or networksetup on macOS
        Ok(get_fallback())
    }
}

pub fn get_fallback() -> NetworkInfo {
    NetworkInfo {
        wifi_adapters: vec![],
        ethernet_adapters: vec![],
        bluetooth_adapters: vec![],
    }
}
