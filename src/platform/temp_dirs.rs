// Platform-specific temporary directory detection
use std::path::Path;

/// Get the Recycle Bin directory path
pub fn get_recycle_bin_directory() -> Option<String> {
    #[cfg(windows)]
    {
        // Get all drives and return the first available Recycle Bin
        let drives = vec!["C:", "D:", "E:", "F:"];
        for drive in drives {
            let recycle_path = format!("{}\\$Recycle.Bin", drive);
            if Path::new(&recycle_path).exists() {
                return Some(recycle_path);
            }
        }
        None
    }

    #[cfg(not(windows))]
    {
        // On Unix-like systems, the trash location varies
        if let Ok(home) = std::env::var("HOME") {
            // Try common trash locations
            let trash_paths = vec![
                format!("{}/.local/share/Trash", home),
                format!("{}/.Trash", home),
            ];

            for path in trash_paths {
                if Path::new(&path).exists() {
                    return Some(path);
                }
            }
        }
        None
    }
}

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

        // NOTE: Prefetch and Recycle Bin are intentionally excluded:
        // - Prefetch: Contains performance optimization files, not temporary files.
        //   Deleting them degrades system performance.
        // Users can manually add these paths if desired using 'msc clean add'.
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
