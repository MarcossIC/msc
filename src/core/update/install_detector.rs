use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum InstallMethod {
    Manual,     // GitHub release o build local
    Winget,     // Windows Package Manager
    Chocolatey, // Chocolatey
    Homebrew,   // macOS Homebrew
    Cargo,      // cargo install
}

/// Detecta el método de instalación basado en la ubicación del binario y el sistema
pub fn detect_install_method() -> Result<InstallMethod> {
    let current_exe = std::env::current_exe()?;
    let exe_path = current_exe.to_string_lossy();

    // Detección específica por plataforma
    #[cfg(windows)]
    {
        detect_windows_install_method(&exe_path)
    }

    #[cfg(target_os = "macos")]
    {
        detect_macos_install_method(&current_exe)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        detect_linux_install_method(&exe_path)
    }
}

#[cfg(windows)]
fn detect_windows_install_method(exe_path: &str) -> Result<InstallMethod> {
    let exe_lower = exe_path.to_lowercase();

    // Verificar WinGet
    // WinGet instala en: %LOCALAPPDATA%\Microsoft\WinGet\Packages\
    if exe_lower.contains("\\microsoft\\winget\\packages\\") {
        return Ok(InstallMethod::Winget);
    }

    // Verificar Chocolatey
    // Chocolatey instala en: %PROGRAMDATA%\chocolatey\bin\ o %PROGRAMDATA%\chocolatey\lib\
    if exe_lower.contains("\\chocolatey\\") {
        return Ok(InstallMethod::Chocolatey);
    }

    // Verificar cargo install
    // Cargo instala en: %USERPROFILE%\.cargo\bin\
    if exe_lower.contains("\\.cargo\\bin\\") {
        return Ok(InstallMethod::Cargo);
    }

    // Si no coincide con ningún patrón conocido, es instalación manual
    Ok(InstallMethod::Manual)
}

#[cfg(target_os = "macos")]
fn detect_macos_install_method(exe_path: &Path) -> Result<InstallMethod> {
    let exe_str = exe_path.to_string_lossy();

    // Verificar Homebrew
    // Homebrew instala en: /opt/homebrew/bin/ (Apple Silicon) o /usr/local/bin/ (Intel)
    // Y generalmente son symlinks
    if exe_str.starts_with("/opt/homebrew/") || exe_str.starts_with("/usr/local/") {
        // Verificar si es un symlink (típico de Homebrew)
        if exe_path.read_link().is_ok() {
            return Ok(InstallMethod::Homebrew);
        }
    }

    // Verificar cargo install
    if let Some(home) = std::env::var_os("HOME") {
        let home_path = Path::new(&home);
        let cargo_bin = home_path.join(".cargo").join("bin");

        if exe_path.starts_with(&cargo_bin) {
            return Ok(InstallMethod::Cargo);
        }
    }

    Ok(InstallMethod::Manual)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn detect_linux_install_method(exe_path: &str) -> Result<InstallMethod> {
    // Verificar cargo install
    // Cargo instala en: ~/.cargo/bin/
    if exe_path.contains("/.cargo/bin/") {
        return Ok(InstallMethod::Cargo);
    }

    // Si está en /usr/local/bin o /usr/bin podría ser manual o package manager
    // Por seguridad, asumimos manual para estos casos
    Ok(InstallMethod::Manual)
}
