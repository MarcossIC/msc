use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::install_detector::UpdateTarget;
use super::platform_installer;
use super::release_info::{fetch_latest_release, get_platform_assets, ReleaseInfo};

pub struct UpdateManager {
    repo: String,
}

impl UpdateManager {
    /// Crea un nuevo UpdateManager
    pub fn new() -> Result<Self> {
        Ok(Self {
            repo: "MarcossIC/msc".to_string(),
        })
    }

    /// Obtiene la versión actual de MSC
    pub fn get_current_version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// Verifica si hay actualizaciones disponibles.
    /// Retorna Some(ReleaseInfo) si hay una nueva versión, None si ya está actualizado.
    pub fn check_for_updates(&self) -> Result<Option<ReleaseInfo>> {
        let current_version = Self::get_current_version();

        println!("{}", "Fetching latest release information...".cyan());

        let release = fetch_latest_release(&self.repo)
            .context("Failed to fetch release information from GitHub")?;

        let latest_version = release.version();

        // Comparar versiones
        if Self::is_newer_version(&latest_version, current_version) {
            Ok(Some(release))
        } else {
            Ok(None)
        }
    }

    /// Descarga la actualización.
    ///
    /// El `target` determina qué asset descargar (MSI o ZIP) y dónde guardar
    /// el archivo temporal:
    /// - `Msi`        → temp en `std::env::temp_dir()` con sufijo `.msi`
    /// - `PortableZip`→ temp en el directorio del ejecutable con sufijo `.zip`
    ///                  (mismo volumen requerido para el rename atómico via self_replace)
    ///
    /// Retorna `(ruta del archivo descargado, hash SHA256 esperado, nombre original del asset)`
    pub fn download_update(
        &self,
        release: &ReleaseInfo,
        target: UpdateTarget,
    ) -> Result<(PathBuf, String, String)> {
        let (binary_asset, checksum_asset) = get_platform_assets(release, target)?;

        println!("{} {}", "Downloading:".cyan(), binary_asset.name.yellow());

        // Descargar el binario
        let binary_data = reqwest::blocking::get(&binary_asset.browser_download_url)
            .context("Failed to download update")?
            .bytes()
            .context("Failed to read update data")?;

        println!(
            "{} {} bytes",
            "Downloaded:".green(),
            binary_data.len().to_string().bold()
        );

        // Descargar el checksum
        println!("{}", "Downloading checksum...".dimmed());
        let checksum_text = reqwest::blocking::get(&checksum_asset.browser_download_url)
            .context("Failed to download checksum")?
            .text()
            .context("Failed to read checksum")?;

        // Parsear el checksum (formato: "<hash>  <filename>")
        let expected_hash = checksum_text
            .split_whitespace()
            .next()
            .ok_or_else(|| anyhow!("Invalid checksum format"))?
            .to_string();

        // Crear archivo temporal con sufijo correcto y en el directorio adecuado.
        // Para PortableZip se usa el directorio del ejecutable actual (mismo volumen)
        // para que self_replace pueda hacer un rename atómico.
        let mut temp_file = match target {
            UpdateTarget::Msi => tempfile::Builder::new()
                .suffix(".msi")
                .tempfile_in(std::env::temp_dir())
                .context("Failed to create temporary MSI file")?,

            UpdateTarget::PortableZip => {
                let exe_path = std::env::current_exe()
                    .context("Failed to determine current executable path")?;
                let exe_dir = exe_path
                    .parent()
                    .ok_or_else(|| anyhow!("Executable has no parent directory"))?
                    .to_path_buf();

                tempfile::Builder::new()
                    .suffix(".zip")
                    .tempfile_in(&exe_dir)
                    .context(
                        "Failed to create temporary ZIP in executable directory. \
                        Make sure the install directory is writable.",
                    )?
            }
        };

        // Escribir datos al archivo temporal
        temp_file
            .write_all(&binary_data)
            .context("Failed to write update to temporary file")?;

        // Sincronizar para asegurar que los datos estén en disco
        temp_file
            .flush()
            .context("Failed to flush temporary file")?;

        // Persistir el archivo temporal (lo mantiene después de que se libere el NamedTempFile)
        // Esto es necesario porque el archivo se usa después en la instalación
        let temp_path = temp_file
            .into_temp_path()
            .keep()
            .context("Failed to persist temporary file")?;

        log::info!(
            "Downloaded update to temporary file: {}",
            temp_path.display()
        );

        Ok((temp_path, expected_hash, binary_asset.name.clone()))
    }

    /// Verifica el checksum SHA256 del archivo descargado
    pub fn verify_checksum(&self, file_path: &Path, expected_hash: &str) -> Result<()> {
        println!("{}", "Verifying checksum...".cyan());

        let file_data =
            fs::read(file_path).context("Failed to read downloaded file for verification")?;

        let mut hasher = Sha256::new();
        hasher.update(&file_data);
        let calculated_hash = hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();

        if calculated_hash.to_lowercase() != expected_hash.to_lowercase() {
            return Err(anyhow!(
                "Checksum mismatch!\nExpected: {}\nGot: {}",
                expected_hash,
                calculated_hash
            ));
        }

        println!("{}", "✓ Checksum verified".green());

        Ok(())
    }

    /// Ejecuta el proceso completo de actualización
    pub fn perform_update(&self, release: &ReleaseInfo, target: UpdateTarget) -> Result<()> {
        // 1. Descargar actualización y checksum
        let (update_file, expected_hash, asset_name) = self.download_update(release, target)?;

        // 2. Verificar checksum
        self.verify_checksum(&update_file, &expected_hash)?;

        // 3. Instalar actualización (específico por plataforma y target)
        println!("\n{}", "Installing update...".cyan());
        platform_installer::install_update(&update_file, &asset_name, target)?;

        println!("\n{}", "━".repeat(40).green());
        println!(
            "  {} {}",
            "Successfully updated to version".green().bold(),
            release.version().yellow().bold()
        );
        println!("{}", "━".repeat(40).green());

        Ok(())
    }

    /// Compara dos versiones semánticas.
    /// Retorna true si la nueva versión es más reciente.
    fn is_newer_version(new: &str, current: &str) -> bool {
        let parse_version =
            |v: &str| -> Vec<u32> { v.split('.').filter_map(|s| s.parse::<u32>().ok()).collect() };

        let new_parts = parse_version(new);
        let current_parts = parse_version(current);

        for (new_part, current_part) in new_parts.iter().zip(current_parts.iter()) {
            if new_part > current_part {
                return true;
            } else if new_part < current_part {
                return false;
            }
        }

        // Si todas las partes coinciden, verificar si la nueva versión tiene más componentes
        new_parts.len() > current_parts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(UpdateManager::is_newer_version("0.2.0", "0.1.9"));
        assert!(UpdateManager::is_newer_version("0.1.10", "0.1.9"));
        assert!(UpdateManager::is_newer_version("1.0.0", "0.9.9"));
        assert!(!UpdateManager::is_newer_version("0.1.9", "0.1.9"));
        assert!(!UpdateManager::is_newer_version("0.1.8", "0.1.9"));
    }

    #[test]
    fn test_current_version() {
        let version = UpdateManager::get_current_version();
        assert!(!version.is_empty());
        assert!(version.contains('.'));
    }
}
