use anyhow::Result;

#[cfg(windows)]
pub fn is_elevated() -> bool {
    use std::mem;
    use std::ptr;
    use winapi::ctypes::c_void;

    unsafe {
        let mut handle: *mut c_void = ptr::null_mut();

        if winapi::um::processthreadsapi::OpenProcessToken(
            winapi::um::processthreadsapi::GetCurrentProcess(),
            winapi::um::winnt::TOKEN_QUERY,
            &mut handle,
        ) == 0
        {
            return false;
        }

        let mut elevation: winapi::um::winnt::TOKEN_ELEVATION = mem::zeroed();
        let mut size: u32 = 0;

        let result = winapi::um::securitybaseapi::GetTokenInformation(
            handle,
            winapi::um::winnt::TokenElevation,
            &mut elevation as *mut _ as *mut c_void,
            mem::size_of::<winapi::um::winnt::TOKEN_ELEVATION>() as u32,
            &mut size,
        );

        winapi::um::handleapi::CloseHandle(handle);

        result != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    // On Unix, check if running as root
    unsafe { libc::geteuid() == 0 }
}

#[cfg(windows)]
pub fn elevate_and_rerun() -> Result<bool> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let exe_path = std::env::current_exe()?;
    let args: Vec<String> = std::env::args().skip(1).collect();

    let result = Command::new("powershell")
        .args([
            "-Command",
            &format!(
                "Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs -Wait",
                exe_path.display(),
                args.join(" ")
            ),
        ])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .status();

    match result {
        Ok(status) => Ok(status.success()),
        Err(_) => Ok(false),
    }
}

#[cfg(not(windows))]
pub fn elevate_and_rerun() -> Result<bool> {
    // On Unix, can't auto-elevate, user must run with sudo
    Ok(false)
}

/// Ensures the program has elevated privileges, attempting to elevate if needed
pub fn ensure_elevated() -> Result<bool> {
    if is_elevated() {
        return Ok(true);
    }

    elevate_and_rerun()
}
