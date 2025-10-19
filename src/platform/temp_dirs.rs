// Platform-specific temporary directory detection
use std::path::Path;

/// Get only the default system temporary directories
pub fn get_default_temp_directories() -> Vec<String> {
    let mut dirs = Vec::new();

    #[cfg(windows)]
    {
        // 1. C:\Windows\Temp (system temp folder)
        if let Ok(windir) = std::env::var("SystemRoot") {
            let windows_temp = format!("{}\\Temp", windir);
            if Path::new(&windows_temp).exists() {
                dirs.push(windows_temp);
            }
        } else {
            let default_windows_temp = "C:\\Windows\\Temp".to_string();
            if Path::new(&default_windows_temp).exists() {
                dirs.push(default_windows_temp);
            }
        }

        // 2. C:\Users\<username>\AppData\Local\Temp (user temp folder)
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            let user_temp = format!("{}\\Temp", localappdata);
            if Path::new(&user_temp).exists() {
                dirs.push(user_temp);
            }
        }
        if let Ok(temp) = std::env::var("TEMP") {
            if !dirs.contains(&temp) && Path::new(&temp).exists() {
                dirs.push(temp);
            }
        }

        // 3. C:\Windows\Prefetch (prefetch folder)
        if let Ok(windir) = std::env::var("SystemRoot") {
            let prefetch = format!("{}\\Prefetch", windir);
            if Path::new(&prefetch).exists() {
                dirs.push(prefetch);
            }
        } else {
            let default_prefetch = "C:\\Windows\\Prefetch".to_string();
            if Path::new(&default_prefetch).exists() {
                dirs.push(default_prefetch);
            }
        }

        // 4. Recycle Bin
        let recycle_bin = "C:\\$Recycle.Bin".to_string();
        if Path::new(&recycle_bin).exists() {
            dirs.push(recycle_bin);
        }
    }

    #[cfg(unix)]
    {
        dirs.push("/tmp".to_string());

        if let Ok(tmpdir) = std::env::var("TMPDIR") {
            if !dirs.contains(&tmpdir) {
                dirs.push(tmpdir);
            }
        }
    }

    // Remove duplicates
    let mut unique_dirs: Vec<String> = dirs.into_iter().filter(|d| Path::new(d).exists()).collect();
    unique_dirs.sort();
    unique_dirs.dedup();

    unique_dirs
}
