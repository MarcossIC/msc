// FFmpegManager - Manages FFmpeg tool for video editing
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::Config;

#[derive(Default)]
pub struct FFmpegManager {
    config: Config,
}

impl FFmpegManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            config: Config::load()?,
        })
    }

    /// Verifica si ffmpeg está instalado
    pub fn is_installed(&self) -> bool {
        if let Some(path) = self.config.get_ffmpeg_path() {
            Path::new(path).exists()
        } else {
            false
        }
    }

    /// Obtiene la ruta del ejecutable de ffmpeg
    pub fn get_binary_path(&self) -> Option<PathBuf> {
        self.config.get_ffmpeg_path().map(PathBuf::from)
    }

    /// Verifica si ffmpeg está disponible en el PATH del sistema
    fn check_system_ffmpeg() -> Option<PathBuf> {
        // Intentar ejecutar ffmpeg -version para verificar si está en el PATH
        let result = Command::new("ffmpeg").arg("-version").output();

        if result.is_ok() {
            // ffmpeg está en el PATH, retornar "ffmpeg" como PathBuf
            Some(PathBuf::from("ffmpeg"))
        } else {
            None
        }
    }

    /// Obtiene la última versión disponible de ffmpeg
    /// Consulta la API de GitHub releases
    pub fn get_latest_version() -> Result<String> {
        println!("{}", "Consultando última versión de ffmpeg...".cyan());

        let client = reqwest::blocking::Client::builder()
            .user_agent("msc-cli")
            .build()?;

        let response = client
            .get("https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest")
            .send()?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Error al consultar releases de ffmpeg: HTTP {}",
                response.status()
            ));
        }

        // Parsear JSON
        let json: serde_json::Value = response.json()?;

        // Extraer tag_name
        let tag = json["tag_name"]
            .as_str()
            .ok_or_else(|| anyhow!("No se pudo extraer tag_name del release"))?;

        println!(
            "{} {}",
            "Última versión encontrada:".green(),
            tag.yellow().bold()
        );

        Ok(tag.to_string())
    }

    /// Descarga el binario de ffmpeg desde GitHub
    /// Descarga el ZIP completo y extrae ffmpeg.exe y ffprobe.exe
    pub fn download_and_extract(version: &str) -> Result<(Vec<u8>, Vec<u8>)> {
        let download_url = format!(
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/{}/ffmpeg-master-latest-win64-gpl-shared.zip",
            version
        );

        println!("{}", "Descargando ffmpeg...".cyan());
        println!("{} {}", "URL:".dimmed(), download_url.dimmed());

        let response =
            reqwest::blocking::get(&download_url).context("Error al descargar ffmpeg")?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Error HTTP {}: No se pudo descargar ffmpeg",
                response.status()
            ));
        }

        let zip_bytes = response.bytes()?;
        println!("{} {} bytes", "Descargado:".green(), zip_bytes.len());

        println!("{}", "Extrayendo archivos...".cyan());

        // Leer el ZIP desde memoria
        let cursor = Cursor::new(zip_bytes);
        let mut archive = zip::ZipArchive::new(cursor).context("Error al leer el archivo ZIP")?;

        let mut ffmpeg_exe: Option<Vec<u8>> = None;
        let mut ffprobe_exe: Option<Vec<u8>> = None;

        // Buscar ffmpeg.exe y ffprobe.exe dentro del ZIP
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let file_name = file.name().to_string();

            // Buscar bin/ffmpeg.exe
            if file_name.ends_with("bin/ffmpeg.exe") {
                println!("{} {}", "Encontrado:".green(), file_name);
                let mut buffer = Vec::new();
                std::io::copy(&mut file, &mut buffer)?;
                ffmpeg_exe = Some(buffer);
            }

            // Buscar bin/ffprobe.exe
            if file_name.ends_with("bin/ffprobe.exe") {
                println!("{} {}", "Encontrado:".green(), file_name);
                let mut buffer = Vec::new();
                std::io::copy(&mut file, &mut buffer)?;
                ffprobe_exe = Some(buffer);
            }

            // Si ya encontramos ambos, podemos parar
            if ffmpeg_exe.is_some() && ffprobe_exe.is_some() {
                break;
            }
        }

        let ffmpeg_data =
            ffmpeg_exe.ok_or_else(|| anyhow!("No se encontró ffmpeg.exe en el archivo ZIP"))?;

        let ffprobe_data =
            ffprobe_exe.ok_or_else(|| anyhow!("No se encontró ffprobe.exe en el archivo ZIP"))?;

        println!("{}", "✓ Archivos extraídos correctamente".green());

        Ok((ffmpeg_data, ffprobe_data))
    }

    /// Instala ffmpeg en el directorio de la aplicación
    pub fn install(&mut self) -> Result<PathBuf> {
        // 1. Obtener última versión
        let version = Self::get_latest_version()?;

        // 2. Descargar y extraer binarios
        let (ffmpeg_data, ffprobe_data) = Self::download_and_extract(&version)?;

        // 3. Crear directorio de instalación
        let install_dir = Self::get_install_dir()?;
        fs::create_dir_all(&install_dir)?;

        // 4. Guardar ejecutables
        let ffmpeg_path = install_dir.join("ffmpeg.exe");
        let ffprobe_path = install_dir.join("ffprobe.exe");

        fs::write(&ffmpeg_path, ffmpeg_data).context("Error al guardar ffmpeg.exe")?;
        fs::write(&ffprobe_path, ffprobe_data).context("Error al guardar ffprobe.exe")?;

        // 5. Guardar ruta en configuración y marcar que fue instalado por msc
        self.config
            .set_ffmpeg_path(ffmpeg_path.to_string_lossy().to_string());
        self.config.set_ffmpeg_installed_by_msc(true);
        self.config.save()?;

        println!("{}", "✓ FFmpeg listo".green());
        println!();

        Ok(ffmpeg_path)
    }

    /// Obtiene el directorio de instalación
    fn get_install_dir() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().context("No se pudo determinar el directorio de configuración")?;

        Ok(config_dir.join("msc").join("bin"))
    }

    /// Asegura que ffmpeg esté instalado y listo para usar
    /// Prioridad:
    /// 1. Verificar si está en el PATH del sistema (transparente, sin mensajes)
    /// 2. Verificar si lo instalamos nosotros (transparente, sin mensajes)
    /// 3. Si no está disponible, instalarlo (con mensaje breve solo la primera vez)
    pub fn ensure_ffmpeg(&mut self) -> Result<PathBuf> {
        // 1. Verificar si ffmpeg está en el PATH del sistema
        if let Some(system_path) = Self::check_system_ffmpeg() {
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
        println!("{}", "🔧 Iniciando FFmpeg (primera ejecución)...".cyan());
        self.install()
    }

    /// Verifica que ffmpeg está funcionando correctamente
    pub fn verify_ffmpeg(&self) -> Result<String> {
        let path = self
            .get_binary_path()
            .ok_or_else(|| anyhow!("ffmpeg path not set, call ensure_ffmpeg() first"))?;

        let output = Command::new(&path)
            .arg("-version")
            .output()
            .map_err(|e| anyhow!("Failed to execute ffmpeg: {}", e))?;

        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(version)
        } else {
            Err(anyhow!(
                "ffmpeg -version failed with status: {}",
                output.status
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_manager_new() {
        let manager = FFmpegManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_system_ffmpeg_detection() {
        let result = FFmpegManager::check_system_ffmpeg();
        match result {
            Some(path) => println!("FFmpeg encontrado: {:?}", path),
            None => println!("FFmpeg no encontrado en el sistema"),
        }
    }

    #[test]
    fn test_get_install_dir() {
        let result = FFmpegManager::get_install_dir();
        assert!(result.is_ok());
        println!("Install dir: {:?}", result.unwrap());
    }
}
