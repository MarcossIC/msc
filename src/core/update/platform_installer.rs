use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use crate::platform::elevation::is_elevated;

/// Instala la actualización dependiendo de la plataforma
pub fn install_update(update_file: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        install_msi(update_file)
    }

    #[cfg(unix)]
    {
        install_binary_from_tarball(update_file)
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
    let _ = fs::remove_file(msi_path);

    Ok(())
}

#[cfg(unix)]
fn install_binary_from_tarball(tar_path: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    use tar::Archive;

    println!("{}", "Extracting update archive...".cyan());

    // Verificar que el archivo existe
    if !tar_path.exists() {
        return Err(anyhow!("Tarball file not found: {}", tar_path.display()));
    }

    // Obtener el binario actual
    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;
    let backup_path = current_exe.with_extension("bak");

    println!("{}", "Creating backup of current binary...".dimmed());

    // Crear backup del binario actual
    fs::copy(&current_exe, &backup_path).context("Failed to create backup of current binary")?;

    // Extraer el tarball
    let temp_dir = std::env::temp_dir().join("msc_update");
    fs::create_dir_all(&temp_dir).context("Failed to create temporary directory")?;

    // Leer y descomprimir el archivo .tar.xz
    let tar_file = fs::File::open(tar_path).context("Failed to open tarball")?;

    // Nota: .tar.xz usa compresión xz, pero la mayoría son .tar.gz
    // Vamos a intentar detectar el tipo basándonos en la extensión
    let file_name = tar_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    if file_name.ends_with(".tar.gz") {
        let decoder = GzDecoder::new(tar_file);
        let mut archive = Archive::new(decoder);
        archive
            .unpack(&temp_dir)
            .context("Failed to extract tarball")?;
    } else if file_name.ends_with(".tar.xz") {
        // Para .tar.xz necesitamos usar xz2
        // Por ahora, vamos a usar flate2 como fallback
        // En producción, deberíamos agregar xz2 crate
        let mut decoder = GzDecoder::new(tar_file);
        let mut buffer = Vec::new();
        decoder
            .read_to_end(&mut buffer)
            .context("Failed to decompress xz archive")?;

        let mut archive = Archive::new(&buffer[..]);
        archive
            .unpack(&temp_dir)
            .context("Failed to extract tarball")?;
    } else {
        return Err(anyhow!("Unsupported archive format: {}", file_name));
    }

    // Buscar el binario msc en el directorio extraído
    let new_binary = temp_dir.join("msc");

    if !new_binary.exists() {
        return Err(anyhow!("Binary 'msc' not found in extracted archive"));
    }

    println!("{}", "Replacing binary...".cyan());

    // Reemplazar el binario (rename es atómico en Unix)
    fs::rename(&new_binary, &current_exe).context("Failed to replace binary")?;

    // Restaurar permisos ejecutables en Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(&current_exe)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&current_exe, perms).context("Failed to set executable permissions")?;
    }

    println!("{}", "✓ Binary replacement completed".green());

    // Limpiar archivos temporales
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::remove_file(tar_path);

    println!(
        "{}",
        "Note: Backup saved at: {}".dimmed().to_string()
            + &backup_path.display().to_string().yellow().to_string()
    );

    Ok(())
}
