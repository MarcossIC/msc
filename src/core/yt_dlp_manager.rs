use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::Config;

pub struct YtDlpManager {
    config: Config,
}

impl YtDlpManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            config: Config::load()?,
        })
    }

    /// Verifica si yt-dlp está instalado
    pub fn is_installed(&self) -> bool {
        if let Some(path) = self.config.get_yt_dlp_path() {
            Path::new(path).exists()
        } else {
            false
        }
    }

    /// Obtiene la ruta del ejecutable de yt-dlp
    pub fn get_binary_path(&self) -> Option<PathBuf> {
        self.config.get_yt_dlp_path().map(PathBuf::from)
    }

    /// Obtiene la última versión disponible de yt-dlp
    /// Estrategia: Hacer petición a /releases/latest y seguir redirect
    pub fn get_latest_version() -> Result<String> {
        println!("{}", "Consultando última versión de yt-dlp...".cyan());

        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;

        let response = client
            .get("https://github.com/yt-dlp/yt-dlp/releases/latest")
            .send()?;

        // Extraer Location header
        if let Some(location) = response.headers().get("Location") {
            let location_str = location.to_str()?;

            // Parsear: https://github.com/yt-dlp/yt-dlp/releases/tag/2025.11.12
            // Extraer: 2025.11.12
            if let Some(version) = location_str.split("/tag/").nth(1) {
                println!(
                    "{} {}",
                    "Última versión encontrada:".green(),
                    version.yellow().bold()
                );
                return Ok(version.to_string());
            }
        }

        Err(anyhow!("No se pudo determinar la última versión"))
    }

    /// Descarga el binario de yt-dlp desde GitHub
    pub fn download_binary(version: &str) -> Result<Vec<u8>> {
        let download_url = format!(
            "https://github.com/yt-dlp/yt-dlp/releases/download/{}/yt-dlp.exe",
            version
        );

        println!("{}", "Descargando yt-dlp...".cyan());
        println!("{} {}", "URL:".dimmed(), download_url.dimmed());

        let response =
            reqwest::blocking::get(&download_url).context("Error al descargar yt-dlp")?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Error HTTP {}: No se pudo descargar yt-dlp",
                response.status()
            ));
        }

        let bytes = response.bytes()?.to_vec();

        println!("{} {} bytes", "Descargado:".green(), bytes.len());

        Ok(bytes)
    }

    /// Instala yt-dlp en el directorio de la aplicación
    pub fn install(&mut self) -> Result<PathBuf> {
        println!();
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
        println!("{}", "  Instalación de yt-dlp".cyan().bold());
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
        println!();

        // 1. Obtener última versión
        let version = Self::get_latest_version()?;

        // 2. Descargar binario
        let binary_data = Self::download_binary(&version)?;

        // 3. Crear directorio de instalación
        let install_dir = Self::get_install_dir()?;
        fs::create_dir_all(&install_dir)?;

        // 4. Guardar ejecutable
        let binary_path = install_dir.join("yt-dlp.exe");
        fs::write(&binary_path, binary_data).context("Error al guardar el ejecutable")?;

        println!(
            "{} {}",
            "Instalado en:".green().bold(),
            binary_path.display()
        );

        // 5. Guardar ruta en configuración y marcar que fue instalado por msc
        self.config
            .set_yt_dlp_path(binary_path.to_string_lossy().to_string());
        self.config.set_yt_dlp_installed_by_msc(true);
        self.config.save()?;

        println!("{}", "✓ Configuración actualizada".green());
        println!("{}", "✓ yt-dlp instalado y registrado por msc".green());
        println!();

        Ok(binary_path)
    }

    /// Obtiene el directorio de instalación
    fn get_install_dir() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().context("No se pudo determinar el directorio de configuración")?;

        Ok(config_dir.join("msc").join("bin"))
    }

    /// Verifica si yt-dlp está disponible en el PATH del sistema
    fn check_system_ytdlp() -> Option<PathBuf> {
        // Intentar ejecutar yt-dlp --version para verificar si está en el PATH
        let result = Command::new("yt-dlp").arg("--version").output();

        if result.is_ok() {
            // yt-dlp está en el PATH, retornar "yt-dlp" como PathBuf
            Some(PathBuf::from("yt-dlp"))
        } else {
            None
        }
    }

    /// Asegura que yt-dlp esté instalado y listo para usar
    /// Prioridad:
    /// 1. Verificar si está en el PATH del sistema
    /// 2. Verificar si lo instalamos nosotros (config.yt_dlp_path)
    /// 3. Si no está disponible, descargarlo e instalarlo
    pub fn ensure_yt_dlp(&mut self) -> Result<PathBuf> {
        // 1. Verificar si yt-dlp está en el PATH del sistema
        if let Some(system_path) = Self::check_system_ytdlp() {
            println!(
                "{} {}",
                "✓ Usando yt-dlp del sistema (PATH):".green(),
                "yt-dlp".cyan()
            );
            return Ok(system_path);
        }

        // 2. Verificar si lo instalamos nosotros previamente
        if self.is_installed() {
            if let Some(path) = self.get_binary_path() {
                println!(
                    "{} {}",
                    "✓ Usando yt-dlp instalado por msc:".green(),
                    path.display().to_string().cyan()
                );
                return Ok(path);
            }
        }

        // 3. No está disponible, necesitamos instalarlo
        println!("{}", "yt-dlp no está instalado.".yellow());
        self.install()
    }
}
