use crate::core::system_info::types::OsInfo;
use crate::error::Result;
use sysinfo::System;

pub fn collect() -> Result<OsInfo> {
    Ok(OsInfo {
        name: System::name().unwrap_or_else(|| "Unknown".to_string()),
        version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
        build: None, // Could get from platform-specific code
        architecture: std::env::consts::ARCH.to_string(),
        kernel_version: System::kernel_version(),
    })
}

pub fn get_fallback() -> OsInfo {
    OsInfo {
        name: "Unknown".to_string(),
        version: "Unknown".to_string(),
        build: None,
        architecture: std::env::consts::ARCH.to_string(),
        kernel_version: None,
    }
}
