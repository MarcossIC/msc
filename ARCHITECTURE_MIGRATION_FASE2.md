### FASE 2: Extraer Código de Plataforma (MEDIO RIESGO)
**Duración estimada**: 2-3 horas
**Riesgo**: Medio
**Objetivo**: Aislar código específico de plataforma (Windows/Unix)

#### Paso 2.1: Extraer platform/elevation.rs
**Origen**: `src/main.rs` funciones `is_elevated()`, `elevate_and_rerun()`
**Destino**: `src/platform/elevation.rs`

```rust
// src/platform/elevation.rs
use anyhow::Result;

#[cfg(windows)]
pub fn is_elevated() -> bool {
    use std::ptr;
    use std::mem;
    use winapi::ctypes::c_void;

    unsafe {
        let mut handle: *mut c_void = ptr::null_mut();

        if winapi::um::processthreadsapi::OpenProcessToken(
            winapi::um::processthreadsapi::GetCurrentProcess(),
            winapi::um::winnt::TOKEN_QUERY,
            &mut handle,
        ) == 0 {
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
        .args(&[
            "-Command",
            &format!(
                "Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs -Wait",
                exe_path.display(),
                args.join(" ")
            )
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
```

**Actualizar src/platform/mod.rs**:
```rust
// src/platform/mod.rs
pub mod elevation;
pub mod temp_dirs;
pub mod fs;

// Re-exports para imports limpios
pub use elevation::{is_elevated, elevate_and_rerun, ensure_elevated};
pub use temp_dirs::get_temp_directories;
pub use fs::is_hidden;
```

#### Paso 2.2: Extraer platform/temp_dirs.rs
**Origen**: `src/main.rs` función `get_temp_directories()`
**Destino**: `src/platform/temp_dirs.rs`

```rust
// src/platform/temp_dirs.rs
use std::path::Path;

/// Get all temporary directories for the current OS
pub fn get_temp_directories() -> Vec<String> {
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
    let mut unique_dirs: Vec<String> = dirs.into_iter()
        .filter(|d| Path::new(d).exists())
        .collect();
    unique_dirs.sort();
    unique_dirs.dedup();

    unique_dirs
}
```

#### Paso 2.3: Extraer platform/fs.rs
**Origen**: `src/main.rs` función `is_hidden_on_windows()`
**Destino**: `src/platform/fs.rs`

```rust
// src/platform/fs.rs

#[cfg(windows)]
pub fn is_hidden(entry: &std::fs::DirEntry) -> bool {
    const FILE_ATTRIBUTE_HIDDEN: u32 = 2;

    if let Ok(metadata) = entry.metadata() {
        use std::os::windows::fs::MetadataExt;
        let attributes = metadata.file_attributes();
        (attributes & FILE_ATTRIBUTE_HIDDEN) != 0
    } else {
        false
    }
}

#[cfg(not(windows))]
pub fn is_hidden(_entry: &std::fs::DirEntry) -> bool {
    false
}
```

#### Paso 2.4: Actualizar imports en main.rs
**En src/main.rs**, reemplazar todas las llamadas a funciones de plataforma:

```rust
// Reemplazar:
is_elevated() → platform::is_elevated()
elevate_and_rerun() → platform::elevate_and_rerun()
get_temp_directories() → platform::get_temp_directories()
is_hidden_on_windows() → platform::is_hidden()
```

#### Paso 2.5: Compilar, validar y commit
```bash
cargo build
cargo test
cargo clippy -- -D warnings
```

**Validación**: 
- ✅ Debe compilar sin errores
- ✅ Sin warnings de Clippy
- ✅ Código de plataforma aislado en módulo `platform/`

```bash
git add .
git commit -m "feat: phase 2 - extract platform-specific code"
```
