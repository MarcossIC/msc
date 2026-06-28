use anyhow::Result;

#[cfg(target_os = "macos")]
use std::path::Path;

/// Método por el que se instaló msc en el sistema
#[derive(Debug, Clone, PartialEq)]
pub enum InstallMethod {
    Manual,      // Instalador MSI descargado manualmente (GitHub release)
    Winget,      // Windows Package Manager
    Chocolatey,  // Chocolatey
    Homebrew,    // macOS Homebrew
    Cargo,       // cargo install
    PortableExe, // Ejecutable portable descargado como .zip (sin instalador)
    Scoop,       // Scoop package manager
}

/// Tipo de asset a descargar para la actualización.
/// Manual/Cargo → MSI; PortableExe → ZIP del ejecutable.
/// Los package managers (Winget, Choco, Homebrew, Scoop) nunca llegan aquí
/// porque retornan antes en `update.rs`, pero se mapean a Msi para completitud.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateTarget {
    Msi,         // Instalador MSI
    PortableZip, // ZIP con el ejecutable portable
}

/// Clasificador PURO del método de instalación en Windows.
///
/// El `path` DEBE estar en minúsculas antes de llamar a esta función
/// (la hace el llamador via `.to_lowercase()`).
///
/// Orden de prioridad:
/// 1. Winget   (`\microsoft\winget\packages\`)
/// 2. Chocolatey (`\chocolatey\`)
/// 3. Cargo    (`\.cargo\bin\`)
/// 4. Manual   (`\program files\msc\bin\` o `\program files (x86)\msc\bin\`)
/// 5. Scoop    (`\scoop\shims\` o `\scoop\apps\`)
/// 6. PortableExe (fallback)
pub fn classify_windows_install_path(path: &str) -> InstallMethod {
    if path.contains("\\microsoft\\winget\\packages\\") {
        return InstallMethod::Winget;
    }
    if path.contains("\\chocolatey\\") {
        return InstallMethod::Chocolatey;
    }
    if path.contains("\\.cargo\\bin\\") {
        return InstallMethod::Cargo;
    }
    if path.contains("\\program files\\msc\\bin\\")
        || path.contains("\\program files (x86)\\msc\\bin\\")
    {
        return InstallMethod::Manual;
    }
    if path.contains("\\scoop\\shims\\") || path.contains("\\scoop\\apps\\") {
        return InstallMethod::Scoop;
    }
    InstallMethod::PortableExe
}

/// Mapea el método de instalación al tipo de asset que se debe descargar
pub fn update_target(method: &InstallMethod) -> UpdateTarget {
    match method {
        InstallMethod::Manual => UpdateTarget::Msi,
        InstallMethod::PortableExe => UpdateTarget::PortableZip,
        InstallMethod::Cargo => UpdateTarget::Msi,
        // Package managers retornan antes en update.rs; mapeamos a Msi por completitud
        InstallMethod::Winget => UpdateTarget::Msi,
        InstallMethod::Chocolatey => UpdateTarget::Msi,
        InstallMethod::Homebrew => UpdateTarget::Msi,
        InstallMethod::Scoop => UpdateTarget::Msi,
    }
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

/// Wrapper IO para Windows: lowercasea la ruta y delega en el clasificador puro
#[cfg(windows)]
fn detect_windows_install_method(exe_path: &str) -> Result<InstallMethod> {
    let exe_lower = exe_path.to_lowercase();
    Ok(classify_windows_install_path(&exe_lower))
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tests para classify_windows_install_path ──────────────────────────────

    #[test]
    fn test_classify_winget() {
        let path = r"c:\users\x\appdata\local\microsoft\winget\packages\msc\msc.exe";
        assert_eq!(classify_windows_install_path(path), InstallMethod::Winget);
    }

    #[test]
    fn test_classify_chocolatey() {
        let path = r"c:\programdata\chocolatey\bin\msc.exe";
        assert_eq!(
            classify_windows_install_path(path),
            InstallMethod::Chocolatey
        );
    }

    #[test]
    fn test_classify_cargo() {
        let path = r"c:\users\marco\.cargo\bin\msc.exe";
        assert_eq!(classify_windows_install_path(path), InstallMethod::Cargo);
    }

    #[test]
    fn test_classify_manual_program_files() {
        let path = r"c:\program files\msc\bin\msc.exe";
        assert_eq!(classify_windows_install_path(path), InstallMethod::Manual);
    }

    #[test]
    fn test_classify_manual_program_files_x86() {
        let path = r"c:\program files (x86)\msc\bin\msc.exe";
        assert_eq!(classify_windows_install_path(path), InstallMethod::Manual);
    }

    #[test]
    fn test_classify_scoop_shims() {
        let path = r"c:\users\marco\scoop\shims\msc.exe";
        assert_eq!(classify_windows_install_path(path), InstallMethod::Scoop);
    }

    #[test]
    fn test_classify_scoop_apps() {
        let path = r"c:\users\marco\scoop\apps\msc\current\msc.exe";
        assert_eq!(classify_windows_install_path(path), InstallMethod::Scoop);
    }

    #[test]
    fn test_classify_portable_exe_downloads() {
        let path = r"c:\users\marco\downloads\msc.exe";
        assert_eq!(
            classify_windows_install_path(path),
            InstallMethod::PortableExe
        );
    }

    /// Verifica el contrato del clasificador: si se pasa un path en mayúsculas
    /// (sin lowercasear), los patrones en minúsculas no coinciden y el resultado
    /// es PortableExe. El llamador es responsable de lowercasear.
    #[test]
    fn test_classify_uppercase_falls_to_portable() {
        let path = r"C:\USERS\MARCO\APPDATA\LOCAL\MICROSOFT\WINGET\PACKAGES\msc.exe";
        assert_eq!(
            classify_windows_install_path(path),
            InstallMethod::PortableExe
        );
    }

    // ── Tests para update_target ──────────────────────────────────────────────

    #[test]
    fn test_update_target_manual_is_msi() {
        assert_eq!(update_target(&InstallMethod::Manual), UpdateTarget::Msi);
    }

    #[test]
    fn test_update_target_portable_exe_is_zip() {
        assert_eq!(
            update_target(&InstallMethod::PortableExe),
            UpdateTarget::PortableZip
        );
    }

    #[test]
    fn test_update_target_cargo_is_msi() {
        assert_eq!(update_target(&InstallMethod::Cargo), UpdateTarget::Msi);
    }
}
