use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, exit};
use serde::Deserialize;
use std::collections::HashMap;

/// Alias data structure matching aliases.json
#[derive(Debug, Deserialize)]
struct AliasConfig {
    aliases: HashMap<String, AliasData>,
}

#[derive(Debug, Deserialize)]
struct AliasData {
    command: String,
}

fn main() {
    // Get the name by which this executable was invoked
    let alias_name = match get_alias_name() {
        Ok(name) => name,
        Err(e) => {
            eprintln!("Error: Failed to determine alias name: {}", e);
            exit(1);
        }
    };

    // Load the configuration
    let config_path = match get_config_path() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: {}", e);
            exit(1);
        }
    };

    let command = match load_command(&config_path, &alias_name) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("Error: {}", e);
            exit(1);
        }
    };

    // Get arguments passed to the alias
    let args: Vec<String> = env::args().skip(1).collect();

    // Build the full command
    let full_command = if args.is_empty() {
        command
    } else {
        format!("{} {}", command, args.join(" "))
    };

    // Execute the command
    let exit_code = execute_command(&full_command);
    exit(exit_code);
}

/// Detect the alias name from the executable name
fn get_alias_name() -> Result<String, String> {
    let exe_path = env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;

    let file_stem = exe_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Failed to get executable name".to_string())?;

    Ok(file_stem.to_string())
}

/// Get the path to aliases.json
fn get_config_path() -> Result<PathBuf, String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| "Could not determine config directory".to_string())?;

    Ok(config_dir
        .join("msc")
        .join("aliases")
        .join("aliases.json"))
}

/// Load the command for a specific alias
fn load_command(config_path: &PathBuf, alias_name: &str) -> Result<String, String> {
    // Check if config file exists
    if !config_path.exists() {
        return Err(format!(
            "Alias configuration not found. Run 'msc alias list' to see available aliases."
        ));
    }

    // Read config file
    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;

    // Parse JSON
    let config: AliasConfig = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config: {}", e))?;

    // Find alias
    config
        .aliases
        .get(alias_name)
        .map(|a| a.command.clone())
        .ok_or_else(|| {
            format!(
                "Alias '{}' not found. Run 'msc alias list' to see available aliases.",
                alias_name
            )
        })
}

/// Execute the command and return exit code
fn execute_command(command: &str) -> i32 {
    #[cfg(target_os = "windows")]
    let status = Command::new("cmd")
        .args(&["/C", command])
        .status();

    #[cfg(not(target_os = "windows"))]
    let status = Command::new("sh")
        .args(&["-c", command])
        .status();

    match status {
        Ok(exit_status) => exit_status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("Failed to execute command: {}", e);
            1
        }
    }
}
