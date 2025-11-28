use anyhow::Result;

/// See docs/security.md for security considerations
fn build_elevation_command(program_path: &str, arguments: &[String]) -> String {
    use base64::{engine::general_purpose, Engine as _};

    let ps_command = if arguments.is_empty() {
        format!(
            "Start-Process -FilePath '{}' -Verb RunAs -Wait",
            program_path
        )
    } else {
        let args_list: Vec<String> = arguments
            .iter()
            .map(|arg| {
                let escaped = arg.replace('\'', "''");
                format!("'{}'", escaped)
            })
            .collect();

        format!(
            "Start-Process -FilePath '{}' -ArgumentList @({}) -Verb RunAs -Wait",
            program_path.replace('\'', "''"),
            args_list.join(", ")
        )
    };

    let utf16_bytes: Vec<u8> = ps_command
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();

    let encoded = general_purpose::STANDARD.encode(&utf16_bytes);

    format!("-EncodedCommand {}", encoded)
}

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

    let powershell_args = build_elevation_command(&exe_path.display().to_string(), &args);

    let mut parts = powershell_args.split_whitespace();
    let encoded_flag = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Missing encoded flag in PowerShell args"))?;
    let encoded_value = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Missing encoded value in PowerShell args"))?;

    let result = Command::new("powershell")
        .args([encoded_flag, encoded_value])
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

pub fn simulate_elevation_command(program: &str, args: &str) -> String {
    build_elevation_command(program, &[args.to_string()])
}
