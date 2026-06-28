use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use super::install_detector::UpdateTarget;

#[cfg(windows)]
use crate::platform::elevation::is_elevated;

#[cfg(windows)]
use std::io::Write;

/// Instala la actualización dependiendo de la plataforma y el tipo de instalación.
///
/// `asset_name` es el nombre original del archivo descargado
/// (ej: "msc-x86_64-pc-windows-msvc.zip", "msc-x86_64-unknown-linux-gnu.tar.xz").
/// En Windows, se ignora `asset_name` ya que se usa la extensión del temp file.
pub fn install_update(update_file: &Path, asset_name: &str, target: UpdateTarget) -> Result<()> {
    #[cfg(windows)]
    {
        let _ = asset_name;
        match target {
            UpdateTarget::Msi => install_msi(update_file),
            UpdateTarget::PortableZip => install_portable_zip(update_file),
        }
    }

    #[cfg(unix)]
    {
        let _ = target;
        install_binary_from_tarball(update_file, asset_name)
    }
}

// ── Funciones puras (no cfg-gateadas) ────────────────────────────────────────
//
// Estas funciones usan solo std + zip (sin Windows APIs), por lo que compilan
// y pueden testearse en cualquier plataforma.

/// Retorna el índice de la primera entrada cuyo nombre termina en `"msc.exe"`.
///
/// Maneja tanto ZIPs planos (`["msc.exe"]`) como anidados
/// (`["msc-x86_64-pc-windows-msvc/msc.exe"]`).
/// Si hay más de una coincidencia, retorna la primera.
pub fn select_exe_entry(names: &[String]) -> Option<usize> {
    names.iter().position(|n| n.ends_with("msc.exe"))
}

/// Extrae los bytes de `msc.exe` desde un ZIP en memoria (bytes crudos).
///
/// Funciona con ZIPs planos (real: `["LICENSE", "msc.exe", "README.md"]`)
/// y con ZIPs anidados (cargo-dist: `["msc-x86_64-pc-windows-msvc/msc.exe"]`).
/// Error si no hay ninguna entrada que termine en `"msc.exe"`.
pub fn extract_exe_from_zip(zip_bytes: &[u8]) -> Result<Vec<u8>> {
    let cursor = io::Cursor::new(zip_bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).context("Error al leer el archivo ZIP de actualización")?;

    // Primera pasada: recolectar los nombres de las entradas
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();

    let idx = select_exe_entry(&names)
        .ok_or_else(|| anyhow!("No se encontró msc.exe dentro del archivo ZIP de actualización"))?;

    let mut file = archive
        .by_index(idx)
        .context("Error al acceder a la entrada msc.exe dentro del ZIP")?;

    let mut buffer = Vec::with_capacity(file.size() as usize);
    io::copy(&mut file, &mut buffer).context("Error al leer los bytes de msc.exe del ZIP")?;

    Ok(buffer)
}

// ── Windows: instalación MSI ─────────────────────────────────────────────────

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

// ── Windows: instalación portable (ZIP → exe en lugar, sin admin) ────────────

/// Extrae `msc.exe` del ZIP y reemplaza el ejecutable en curso, sin admin.
///
/// Flujo:
/// 1. Lee el ZIP del disco
/// 2. Extrae los bytes de msc.exe en memoria
/// 3. Escribe el nuevo exe en un temporal dentro del mismo directorio (mismo volumen)
/// 4. Llama a `replace_with_retry` para el swap atómico via self_replace
/// 5. Limpia el ZIP temporal
#[cfg(windows)]
fn install_portable_zip(zip_path: &Path) -> Result<()> {
    println!("{}", "Extrayendo ejecutable del ZIP de actualización...".cyan());

    // Leer el ZIP completo desde disco
    let zip_bytes =
        fs::read(zip_path).context("Error al leer el archivo ZIP de actualización")?;

    // Extraer msc.exe del ZIP en memoria (puro, sin IO de disco)
    let exe_bytes =
        extract_exe_from_zip(&zip_bytes).context("Error al extraer msc.exe del ZIP")?;

    println!(
        "{} {} bytes",
        "Extraído:".green(),
        exe_bytes.len().to_string().bold()
    );

    // Directorio del ejecutable actual (mismo volumen → rename atómico posible)
    let current_exe = std::env::current_exe()
        .context("No se pudo determinar la ruta del ejecutable actual")?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| anyhow!("El ejecutable no tiene directorio padre"))?
        .to_path_buf();

    // Escribir el nuevo ejecutable en un temporal dentro del mismo directorio
    let mut tmp = tempfile::Builder::new()
        .suffix(".exe")
        .tempfile_in(&exe_dir)
        .context(
            "No se pudo crear un archivo temporal en el directorio de instalación. \
            Verificá que el directorio sea escribible o mové msc a una carpeta de usuario.",
        )?;

    tmp.write_all(&exe_bytes)
        .context("Error al escribir el nuevo ejecutable temporal")?;

    tmp.flush()
        .context("Error al vaciar el buffer del ejecutable temporal")?;

    // Persistir el temporal (no se elimina al salir del scope)
    let tmp_path = tmp
        .into_temp_path()
        .keep()
        .context("Error al persistir el archivo temporal del ejecutable")?;

    println!("{}", "Reemplazando ejecutable en curso...".cyan());
    replace_with_retry(&tmp_path)?;

    println!("{}", "✓ Ejecutable reemplazado correctamente".green());

    // Limpiar el ZIP temporal
    if let Err(e) = fs::remove_file(zip_path) {
        log::warn!(
            "No se pudo eliminar el ZIP temporal {:?}: {}",
            zip_path,
            e
        );
    }

    Ok(())
}

/// Reemplaza el ejecutable en curso con `new_exe` usando `self_replace`.
///
/// Reintenta hasta 3 veces si el error es `PermissionDenied` (AV o handle abierto),
/// con esperas de 250 ms → 500 ms → 1000 ms entre intentos.
/// Cualquier otro error de IO se propaga inmediatamente.
#[cfg(windows)]
fn replace_with_retry(new_exe: &Path) -> Result<()> {
    let delays_ms: [u64; 3] = [250, 500, 1000];
    let mut last_err: Option<io::Error> = None;

    for (attempt, &delay_ms) in delays_ms.iter().enumerate() {
        match self_replace::self_replace(new_exe) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                log::warn!(
                    "Reemplazo del ejecutable bloqueado (intento {}/3): {}",
                    attempt + 1,
                    e
                );
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                last_err = Some(e);
            }
            Err(e) => {
                return Err(anyhow::Error::from(e))
                    .context("Error inesperado al reemplazar el ejecutable");
            }
        }
    }

    let err_msg = last_err
        .map(|e| e.to_string())
        .unwrap_or_else(|| "error desconocido".to_string());

    Err(anyhow!(
        "No se pudo reemplazar el ejecutable después de 3 intentos: {}.\n\
        Es posible que el antivirus u otra instancia de msc esté bloqueando el archivo.\n\
        Cerrá otras instancias de msc y volvé a intentarlo.",
        err_msg
    ))
}

// ── Unix: instalación desde tarball ──────────────────────────────────────────

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

// ── Tests (corren en todas las plataformas) ───────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── Tests para select_exe_entry ───────────────────────────────────────────

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Caso real: ZIP plano tal como lo distribuye cargo-dist para msc
    /// `["LICENSE", "msc.exe", "README.md"]` → Some(1)
    #[test]
    fn test_select_exe_flat_real_case() {
        let names = s(&["LICENSE", "msc.exe", "README.md"]);
        assert_eq!(select_exe_entry(&names), Some(1));
    }

    /// Caso anidado (cargo-dist nested layout)
    #[test]
    fn test_select_exe_nested_case() {
        let names = s(&["msc-x86_64-pc-windows-msvc/msc.exe"]);
        assert_eq!(select_exe_entry(&names), Some(0));
    }

    /// Sin coincidencia → None
    #[test]
    fn test_select_exe_no_match() {
        let names = s(&["README.md", "LICENSE", "CHANGELOG.md"]);
        assert_eq!(select_exe_entry(&names), None);
    }

    /// Entrada vacía → None
    #[test]
    fn test_select_exe_empty() {
        assert_eq!(select_exe_entry(&[]), None);
    }

    /// Dos coincidencias → primera
    #[test]
    fn test_select_exe_two_matches_returns_first() {
        let names = s(&["msc.exe", "backup/msc.exe"]);
        assert_eq!(select_exe_entry(&names), Some(0));
    }

    // ── Tests para extract_exe_from_zip ──────────────────────────────────────

    /// Construye un ZIP en memoria con una entrada en directorio (caso anidado)
    fn make_zip_with_nested_exe(content: &[u8]) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut buf);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("dir/msc.exe", options)
                .expect("start_file failed en fixture");
            zip.write_all(content).expect("write_all failed en fixture");
            zip.finish().expect("finish failed en fixture");
        }
        buf
    }

    /// Construye un ZIP plano (caso real de msc release)
    fn make_zip_flat(content: &[u8]) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut buf);
            let mut zip = zip::ZipWriter::new(cursor);
            let opt = zip::write::SimpleFileOptions::default();
            zip.start_file("LICENSE", opt)
                .expect("start_file LICENSE failed");
            zip.write_all(b"MIT License").expect("write LICENSE failed");
            let opt = zip::write::SimpleFileOptions::default();
            zip.start_file("msc.exe", opt)
                .expect("start_file msc.exe failed");
            zip.write_all(content).expect("write msc.exe failed");
            let opt = zip::write::SimpleFileOptions::default();
            zip.start_file("README.md", opt)
                .expect("start_file README failed");
            zip.write_all(b"readme").expect("write README failed");
            zip.finish().expect("finish failed en fixture");
        }
        buf
    }

    /// Construye un ZIP sin msc.exe
    fn make_zip_no_exe() -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut buf);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("README.md", options)
                .expect("start_file failed en fixture");
            zip.write_all(b"readme content")
                .expect("write_all failed en fixture");
            zip.finish().expect("finish failed en fixture");
        }
        buf
    }

    #[test]
    fn test_extract_exe_from_zip_nested_ok() {
        let zip_bytes = make_zip_with_nested_exe(b"fake-exe-bytes");
        let result = extract_exe_from_zip(&zip_bytes).expect("debería extraer msc.exe");
        assert_eq!(result, b"fake-exe-bytes");
    }

    #[test]
    fn test_extract_exe_from_zip_flat_ok() {
        let zip_bytes = make_zip_flat(b"exe-bytes-flat");
        let result = extract_exe_from_zip(&zip_bytes).expect("debería extraer msc.exe del ZIP plano");
        assert_eq!(result, b"exe-bytes-flat");
    }

    #[test]
    fn test_extract_exe_from_zip_no_match_returns_err() {
        let zip_bytes = make_zip_no_exe();
        let result = extract_exe_from_zip(&zip_bytes);
        assert!(
            result.is_err(),
            "se esperaba Err cuando no hay msc.exe en el ZIP"
        );
    }

    #[test]
    fn test_extract_exe_from_zip_invalid_bytes_returns_err() {
        let result = extract_exe_from_zip(b"esto no es un zip valido");
        assert!(result.is_err(), "se esperaba Err con bytes inválidos");
    }
}
