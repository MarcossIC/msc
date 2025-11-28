// WgetManager - Manages wget tool for web page downloads

use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::Config;

pub struct WgetManager {
    config: Config,
}

impl WgetManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            config: Config::load()?,
        })
    }

    /// Verifica si wget está instalado
    pub fn is_installed(&self) -> bool {
        if let Some(path) = self.config.get_wget_path() {
            Path::new(path).exists()
        } else {
            false
        }
    }

    /// Obtiene la ruta del ejecutable de wget
    pub fn get_binary_path(&self) -> Option<PathBuf> {
        self.config.get_wget_path().map(PathBuf::from)
    }

    /// Verifica si wget está disponible en el PATH del sistema
    fn check_system_wget() -> Option<PathBuf> {
        // Intentar ejecutar wget --version para verificar si está en el PATH
        let result = Command::new("wget").arg("--version").output();

        if result.is_ok() {
            // wget está en el PATH, retornar "wget" como PathBuf
            Some(PathBuf::from("wget"))
        } else {
            None
        }
    }

    /// Descarga el binario de wget desde eternallybored.org
    /// Descarga el ZIP y extrae wget.exe
    pub fn download_and_extract() -> Result<Vec<u8>> {
        // URL del wget precompilado para Windows (versión estable)
        let download_url = "https://eternallybored.org/misc/wget/1.21.4/64/wget.exe";

        println!("{}", "Descargando wget...".cyan());
        println!("{} {}", "URL:".dimmed(), download_url.dimmed());

        let response = reqwest::blocking::get(download_url).context("Error al descargar wget")?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Error HTTP {}: No se pudo descargar wget",
                response.status()
            ));
        }

        let wget_data = response.bytes()?.to_vec();
        println!("{} {} bytes", "Descargado:".green(), wget_data.len());

        Ok(wget_data)
    }

    /// Instala wget en el directorio de la aplicación
    pub fn install(&mut self) -> Result<PathBuf> {
        // 1. Descargar binario
        let wget_data = Self::download_and_extract()?;

        // 2. Crear directorio de instalación
        let install_dir = Self::get_install_dir()?;
        fs::create_dir_all(&install_dir)?;

        // 3. Guardar ejecutable
        let wget_path = install_dir.join("wget.exe");
        fs::write(&wget_path, wget_data).context("Error al guardar wget.exe")?;

        // 4. Guardar ruta en configuración y marcar que fue instalado por msc
        self.config
            .set_wget_path(wget_path.to_string_lossy().to_string());
        self.config.set_wget_installed_by_msc(true);
        self.config.save()?;

        println!("{}", "✓ wget listo".green());
        println!();

        Ok(wget_path)
    }

    /// Obtiene el directorio de instalación
    fn get_install_dir() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().context("No se pudo determinar el directorio de configuración")?;

        Ok(config_dir.join("msc").join("bin"))
    }

    /// Asegura que wget esté instalado y listo para usar
    /// Prioridad:
    /// 1. Verificar si está en el PATH del sistema (transparente, sin mensajes)
    /// 2. Verificar si lo instalamos nosotros (transparente, sin mensajes)
    /// 3. Si no está disponible, instalarlo (con mensaje breve solo la primera vez)
    pub fn ensure_wget(&mut self) -> Result<PathBuf> {
        // 1. Verificar si wget está en el PATH del sistema
        if let Some(system_path) = Self::check_system_wget() {
            // Transparente: usar sin mostrar mensajes
            return Ok(system_path);
        }

        // 2. Verificar si lo instalamos nosotros previamente
        if self.is_installed() {
            if let Some(path) = self.get_binary_path() {
                // Transparente: usar sin mostrar mensajes
                return Ok(path);
            }
        }

        // 3. No está disponible, necesitamos instalarlo
        // Solo mostrar mensaje breve la primera vez
        println!();
        println!("{}", "🔧 Iniciando wget (primera ejecución)...".cyan());
        self.install()
    }

    /// Verifica que wget está funcionando correctamente
    pub fn verify_wget(&self) -> Result<String> {
        let path = self
            .get_binary_path()
            .ok_or_else(|| anyhow!("wget path not set, call ensure_wget() first"))?;

        let output = Command::new(&path)
            .arg("--version")
            .output()
            .map_err(|e| anyhow!("Failed to execute wget: {}", e))?;

        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(version)
        } else {
            Err(anyhow!(
                "wget --version failed with status: {}",
                output.status
            ))
        }
    }
}

impl Default for WgetManager {
    fn default() -> Self {
        Self {
            config: Config::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wget_manager_new() {
        let manager = WgetManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_wget_manager_default() {
        let manager = WgetManager::default();
        assert!(manager.get_binary_path().is_none());
    }

    #[test]
    fn test_system_wget_detection() {
        let result = WgetManager::check_system_wget();
        match result {
            Some(path) => println!("wget found at: {:?}", path),
            None => println!("wget not found in system PATH"),
        }
    }

    #[test]
    fn test_get_install_dir() {
        let result = WgetManager::get_install_dir();
        assert!(result.is_ok());
        println!("Install dir: {:?}", result.unwrap());
    }
}
