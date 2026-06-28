use crate::core::system_info::types::OsInfo;
use crate::error::Result;
use sysinfo::System;

pub fn collect() -> Result<OsInfo> {
    // Cross-platform basics from sysinfo.
    let sys_name = System::name().unwrap_or_else(|| "Unknown".to_string());
    let sys_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
    let architecture = std::env::consts::ARCH.to_string();
    let kernel_version = System::kernel_version();
    let uptime_secs = Some(System::uptime());

    #[cfg(windows)]
    {
        // Enrich with honest Windows details: derived generation/edition, full
        // build (CurrentBuild.UBR), HVCI state and the current user. `name` falls
        // back to the sysinfo string only when the generation can't be derived.
        let win = crate::platform::system::windows::os::get_windows_os_info();
        Ok(OsInfo {
            name: win.name.unwrap_or(sys_name),
            version: sys_version,
            edition: win.edition,
            display_version: win.display_version,
            build: win.build,
            architecture,
            kernel_version,
            hvci: win.hvci,
            current_user: win.current_user,
            uptime_secs,
        })
    }

    #[cfg(not(windows))]
    {
        Ok(OsInfo {
            name: sys_name,
            version: sys_version,
            edition: None,
            display_version: None,
            build: None,
            architecture,
            kernel_version,
            hvci: None,
            current_user: None,
            uptime_secs,
        })
    }
}

pub fn get_fallback() -> OsInfo {
    OsInfo {
        name: "Unknown".to_string(),
        version: "Unknown".to_string(),
        edition: None,
        display_version: None,
        build: None,
        architecture: std::env::consts::ARCH.to_string(),
        kernel_version: None,
        hvci: None,
        current_user: None,
        uptime_secs: None,
    }
}
