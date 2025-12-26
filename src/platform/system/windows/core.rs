use serde::de::DeserializeOwned;
use crate::error::{MscError, Result};

pub fn run_powershell_json<T: DeserializeOwned>(command: &str) -> Result<T> {
    use std::process::Command;
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", command])
        .output()
        .map_err(|e| MscError::other(format!("PowerShell execution failed: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    serde_json::from_str(&stdout)
        .map_err(|e| MscError::other(format!("JSON parsing failed: {e}. Output: {stdout}")))
}