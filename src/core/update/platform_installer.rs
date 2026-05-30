use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use crate::platform::elevation::is_elevated;

/// Instala la actualización dependiendo de la plataforma
/// `asset_name` es el nombre original del archivo descargado (ej: "msc-x86_64-unknown-linux-gnu.tar.xz")
pub fn install_update(update_file: &Path, asset_name: &str) -> Result<()> {
    #[cfg(windows)]
    {
        let _ = asset_name;
        install_msi(update_file)
    }

    #[cfg(unix)]
    {
        install_binary_from_tarball(update_file, asset_name)
    }
}

#[cfg(windows)]
fn install_msi(msi_path: &Path) -> Result<()> {
    println!("{}", "Installing MSI package...".cyan());

    // Verificar si el archivo MSI existe
    if !msi_path.exists() {
        return Err(anyhow!("MSI file not found: {}", msi_path.display()));
    }

    // Preparar el comando msiexec
    let msi_path_str = msi_path.to_string_lossy().to_string();

    if !is_elevated() {
        println!("{}", "Administrator privileges required...".yellow());
        println!("{}", "Requesting elevation...".dimmed());

        // Construir comando PowerShell para ejecutar msiexec con elevación
        let ps_command = format!(
            "Start-Process msiexec -ArgumentList '/i', '{}', '/passive' -Verb RunAs -Wait",
            msi_path_str.replace('\'', "''")
        );

        // Ejecutar PowerShell con el comando elevado
        let status = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_command])
            .status()
            .context("Failed to execute PowerShell elevation command")?;

        if !status.success() {
            return Err(anyhow!("MSI installation was cancelled or failed"));
        }
    } else {
        // Ya tenemos elevación, ejecutar msiexec directamente
        println!("{}", "Running MSI installer...".cyan());

        let status = Command::new("msiexec")
            .args(["/i", &msi_path_str, "/passive"])
            .status()
            .context("Failed to execute msiexec")?;

        if !status.success() {
            return Err(anyhow!(
                "MSI installation failed with exit code: {:?}",
                status.code()
            ));
        }
    }

    println!("{}", "✓ MSI installation completed".green());

    // Limpiar el archivo MSI temporal
    if let Err(e) = fs::remove_file(msi_path) {
        log::warn!("Failed to delete temporary MSI file {:?}: {}", msi_path, e);
    }

    Ok(())
}

#[cfg(unix)]
fn install_binary_from_tarball(tar_path: &Path, asset_name: &str) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;
    use tempfile::tempdir;
    use xz2::read::XzDecoder;

    println!("{}", "Extracting update archive...".cyan());

    if !tar_path.exists() {
        return Err(anyhow!("Tarball file not found: {}", tar_path.display()));
    }

    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;
    let backup_path = current_exe.with_extension("bak");

    println!("{}", "Creating backup of current binary...".dimmed());
    fs::copy(&current_exe, &backup_path).context("Failed to create backup of current binary")?;

    let temp_dir = tempdir().context("Failed to create temporary directory")?;
    let temp_path = temp_dir.path();

    log::info!(
        "Extracting update to secure temporary directory: {}",
        temp_path.display()
    );

    let tar_file = fs::File::open(tar_path).context("Failed to open tarball")?;

    // Usar el nombre original del asset para detectar el formato del archivo
    // (el archivo temporal no conserva la extensión original)
    if asset_name.ends_with(".tar.gz") || asset_name.ends_with(".tgz") {
        let decoder = GzDecoder::new(tar_file);
        let mut archive = Archive::new(decoder);
        archive
            .unpack(temp_path)
            .context("Failed to extract .tar.gz archive")?;
    } else if asset_name.ends_with(".tar.xz") {
        let decoder = XzDecoder::new(tar_file);
        let mut archive = Archive::new(decoder);
        archive
            .unpack(temp_path)
            .context("Failed to extract .tar.xz archive")?;
    } else {
        return Err(anyhow!("Unsupported archive format: {}", asset_name));
    }

    // cargo-dist extracts into a subdirectory, find the binary
    let new_binary = find_binary_in_dir(temp_path, "msc")?;

    println!("{}", "Replacing binary...".cyan());

    fs::rename(&new_binary, &current_exe)
        .or_else(|_| {
            // rename can fail across filesystems, fallback to copy+delete
            fs::copy(&new_binary, &current_exe)?;
            fs::remove_file(&new_binary)?;
            Ok::<(), std::io::Error>(())
        })
        .context("Failed to replace binary")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&current_exe)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&current_exe, perms).context("Failed to set executable permissions")?;
    }

    println!("{}", "✓ Binary replacement completed".green());

    // temp_dir auto-cleans on drop
    if let Err(e) = fs::remove_file(tar_path) {
        log::warn!("Failed to delete temporary tarball {:?}: {}", tar_path, e);
    }

    println!(
        "{}",
        format!("Note: Backup saved at: {}", backup_path.display())
            .dimmed()
            .to_string()
    );

    Ok(())
}

/// Searches for the binary inside extracted directory (handles cargo-dist subdirectories)
#[cfg(unix)]
fn find_binary_in_dir(dir: &Path, name: &str) -> Result<std::path::PathBuf> {
    // Check directly in the directory
    let direct = dir.join(name);
    if direct.exists() {
        return Ok(direct);
    }

    // cargo-dist typically extracts into a subdirectory like "msc-x86_64-unknown-linux-gnu/"
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let candidate = entry.path().join(name);
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }
    }

    Err(anyhow!(
        "Binary '{}' not found in extracted archive at {}",
        name,
        dir.display()
    ))
}
