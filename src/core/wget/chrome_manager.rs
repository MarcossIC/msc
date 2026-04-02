// Chrome Manager - Intelligent Chrome state detection and cookie extraction orchestration
//
// Strategies (in priority order):
// 1. ExistingCDP  — Chrome already running with CDP (user started it manually)
// 2. IsolatedCDP  — Launch a SEPARATE headless Chrome on a dynamic port
// 3. DirectDatabase — Read cookies from disk (fallback, fails on Chrome 127+ ABE)
//
// KEY GUARANTEE: This module NEVER kills the user's browser.

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;
use std::time::Duration;
use sysinfo::{ProcessRefreshKind, System};

use super::cdp_cookies;
use super::chrome_launcher::IsolatedChrome;
use super::wget_cookies::{extract_cookies_from_db, Cookie};

const LEGACY_CDP_PORT: u16 = 9222;

/// Chrome process state detection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChromeState {
    /// Chrome is not running
    NotRunning,
    /// Chrome is running with CDP enabled (on legacy port 9222)
    RunningWithCDP,
    /// Chrome is running but without CDP
    RunningWithoutCDP,
}

/// Extraction strategy to use
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtractionStrategy {
    /// Use existing CDP connection (legacy port 9222)
    ExistingCDP,
    /// Launch an isolated headless Chrome on a dynamic port
    IsolatedCDP,
    /// Direct database extraction (fallback for Chrome < 127)
    DirectDatabase,
}

/// Chrome Manager - orchestrates cookie extraction with intelligent fallback
pub struct ChromeManager {
    browser_type: String,
    db_path: PathBuf,
}

impl ChromeManager {
    pub fn new(browser: &str, db_path: PathBuf) -> Self {
        Self {
            browser_type: browser.to_string(),
            db_path,
        }
    }

    /// Detect current Chrome state
    pub fn detect_chrome_state(&self) -> ChromeState {
        // Check if CDP port is open (user may have started Chrome with --remote-debugging-port=9222)
        if Self::is_cdp_available_sync() {
            return ChromeState::RunningWithCDP;
        }

        // Check if Chrome process is running (without CDP)
        let mut system = System::new();
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );

        let chrome_names = match self.browser_type.as_str() {
            "edge" => vec!["msedge.exe", "msedge"],
            _ => vec![
                "chrome.exe",
                "chrome",
                "google-chrome",
                "google-chrome-stable",
            ],
        };

        for process in system.processes().values() {
            let process_name = process.name().to_string_lossy().to_ascii_lowercase();
            if chrome_names.iter().any(|name| process_name.contains(name)) {
                return ChromeState::RunningWithoutCDP;
            }
        }

        ChromeState::NotRunning
    }

    /// Check if CDP is available on the legacy port (synchronous)
    fn is_cdp_available_sync() -> bool {
        std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", LEGACY_CDP_PORT).parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok()
    }

    /// Determine the best extraction strategy based on flags and Chrome state.
    ///
    /// # Strategy selection
    /// - CDP already available → use it directly (`ExistingCDP`)
    /// - `auto_launch` enabled → launch isolated Chrome (`IsolatedCDP`)
    /// - Otherwise → direct database read (`DirectDatabase`)
    pub fn determine_strategy(
        &self,
        use_cdp: bool,
        auto_launch: bool,
    ) -> (ExtractionStrategy, ChromeState) {
        let state = self.detect_chrome_state();

        let strategy = match (use_cdp, auto_launch, state) {
            // CDP available → use it directly (no need to launch anything)
            (_, _, ChromeState::RunningWithCDP) => ExtractionStrategy::ExistingCDP,

            // auto-launch enabled → launch isolated Chrome (NEVER kills user's browser)
            (_, true, _) => ExtractionStrategy::IsolatedCDP,

            // User wants CDP but Chrome doesn't have it
            (true, false, _) => ExtractionStrategy::ExistingCDP,

            // Fallback: direct database (only works on Chrome < 127)
            _ => ExtractionStrategy::DirectDatabase,
        };

        (strategy, state)
    }

    /// Extract cookies using the best available strategy with automatic fallback.
    pub async fn extract_cookies_smart(
        &self,
        domain: &str,
        use_cdp: bool,
        auto_launch: bool,
    ) -> Result<Vec<Cookie>> {
        let (strategy, chrome_state) = self.determine_strategy(use_cdp, auto_launch);

        println!();
        println!("{}", "🔍 Analizando estado de Chrome...".cyan());

        match chrome_state {
            ChromeState::NotRunning => {
                println!("{}", "   • Chrome no está ejecutándose".dimmed());
            }
            ChromeState::RunningWithCDP => {
                println!(
                    "{}",
                    "   • Chrome ejecutándose con CDP habilitado ✓".green()
                );
            }
            ChromeState::RunningWithoutCDP => {
                println!("{}", "   • Chrome ejecutándose (sin CDP)".yellow());
            }
        }

        let (strategy_name, strategy_desc) = match strategy {
            ExtractionStrategy::ExistingCDP => (
                "Conexión a CDP existente",
                "Usando Chrome que ya está corriendo con CDP",
            ),
            ExtractionStrategy::IsolatedCDP => (
                "Chrome aislado con CDP",
                "Se lanzará un Chrome headless separado (tu navegador NO se cierra)",
            ),
            ExtractionStrategy::DirectDatabase => (
                "Extracción directa de base de datos",
                if chrome_state == ChromeState::RunningWithoutCDP {
                    "Leyendo cookies directamente del disco (puede fallar en Chrome 127+ con ABE)"
                } else {
                    "Leyendo cookies directamente del disco"
                },
            ),
        };
        println!("{} {}", "   • Estrategia:".cyan(), strategy_name.bold());
        println!("{}", format!("     {}", strategy_desc).dimmed());
        println!();

        let result = self
            .execute_strategy_with_fallback(domain, strategy, chrome_state)
            .await;

        match result {
            Ok(cookies) if !cookies.is_empty() => Ok(cookies),
            Ok(_) => Ok(vec![]),
            Err(e) => self.provide_helpful_error(e, chrome_state, use_cdp, auto_launch),
        }
    }

    /// Execute extraction strategy with intelligent fallback
    async fn execute_strategy_with_fallback(
        &self,
        domain: &str,
        strategy: ExtractionStrategy,
        chrome_state: ChromeState,
    ) -> Result<Vec<Cookie>> {
        match strategy {
            ExtractionStrategy::ExistingCDP => {
                match self.try_cdp_extraction_on(LEGACY_CDP_PORT, domain).await {
                    Ok(cookies) => return Ok(cookies),
                    Err(e) => {
                        eprintln!("{}", format!("⚠️  CDP falló: {}", e).yellow());
                        eprintln!("{}", "   Intentando método alternativo...".dimmed());
                    }
                }

                if chrome_state == ChromeState::RunningWithoutCDP {
                    eprintln!(
                        "{}",
                        "   Chrome está abierto. Algunas cookies recientes pueden no estar en disco."
                            .yellow()
                    );
                }

                self.try_database_extraction(domain)
            }

            ExtractionStrategy::IsolatedCDP => {
                match self.try_isolated_cdp_extraction(domain).await {
                    Ok(cookies) => return Ok(cookies),
                    Err(e) => {
                        eprintln!("{}", format!("⚠️  Chrome aislado falló: {}", e).yellow());
                        eprintln!("{}", "   Intentando método alternativo...".dimmed());
                    }
                }

                self.try_database_extraction(domain)
            }

            ExtractionStrategy::DirectDatabase => self.try_database_extraction(domain),
        }
    }

    /// Try CDP extraction on a specific port with retries
    async fn try_cdp_extraction_on(&self, port: u16, domain: &str) -> Result<Vec<Cookie>> {
        println!(
            "{}",
            format!("⟳ Extrayendo cookies via CDP (puerto {})...", port).cyan()
        );

        if !cdp_cookies::is_cdp_available_on(port).await {
            return Err(anyhow::anyhow!("CDP no está disponible en puerto {}", port));
        }

        cdp_cookies::extract_cookies_cdp_with_retry_on(port, domain, 3).await
    }

    /// Launch an isolated Chrome and extract cookies via its dynamic CDP port.
    ///
    /// This is the core of the new architecture:
    /// - Launches a SEPARATE headless Chrome on a random port
    /// - NEVER kills the user's browser
    /// - Uses the original profile for ABE compatibility
    /// - Extracts cookies via CDP Storage API
    /// - Chrome is automatically cleaned up when done
    async fn try_isolated_cdp_extraction(&self, domain: &str) -> Result<Vec<Cookie>> {
        println!();
        println!(
            "{}",
            "🚀 Lanzando Chrome aislado para extracción de cookies..."
                .cyan()
                .bold()
        );
        println!("{}", "   (Tu navegador NO se cerrará)".green());
        println!();

        // Launch isolated Chrome — gets its own dynamic port
        let chrome = IsolatedChrome::launch(&self.browser_type)?;
        let port = chrome.cdp_port();

        // Give Chrome time to load cookies into memory
        println!(
            "{}",
            "   ⏳ Esperando que Chrome cargue las cookies en memoria...".dimmed()
        );

        for _ in 1..=3 {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            print!(".");
            use std::io::{self, Write};
            io::stdout().flush().unwrap();
        }
        println!();
        println!();

        // Extract cookies via CDP on the isolated instance's port
        let result = self.try_cdp_extraction_on(port, domain).await;

        // IsolatedChrome is dropped here — kills only its own process
        drop(chrome);

        result
    }

    /// Try direct database extraction
    fn try_database_extraction(&self, domain: &str) -> Result<Vec<Cookie>> {
        println!("{}", "⟳ Extrayendo cookies de base de datos...".cyan());
        extract_cookies_from_db(&self.db_path, domain)
    }

    /// Provide helpful error message based on context
    fn provide_helpful_error(
        &self,
        error: anyhow::Error,
        chrome_state: ChromeState,
        use_cdp: bool,
        auto_launch: bool,
    ) -> Result<Vec<Cookie>> {
        eprintln!();
        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".red());
        eprintln!("{}", "  EXTRACCIÓN DE COOKIES FALLÓ".red().bold());
        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".red());
        eprintln!();
        eprintln!("{} {}", "Error:".red().bold(), error);
        eprintln!();

        eprintln!("{}", "💡 SOLUCIONES SUGERIDAS:".cyan().bold());
        eprintln!();

        match chrome_state {
            ChromeState::RunningWithoutCDP if use_cdp || auto_launch => {
                eprintln!("{}", "Chrome está abierto sin CDP.".yellow());
                eprintln!();
                eprintln!(
                    "{}",
                    "El perfil puede estar bloqueado por la instancia activa.".dimmed()
                );
                eprintln!();
                eprintln!("{}", "Opción 1: Cerrar Chrome y reintentar".green());
                eprintln!(
                    "{}",
                    "  Cierra Chrome, luego: msc wget cookies <URL> --auto-launch".dimmed()
                );
                eprintln!();
                eprintln!("{}", "Opción 2: Iniciar Chrome manualmente con CDP".green());
                eprintln!("{}", "  chrome.exe --remote-debugging-port=9222".cyan());
                eprintln!("{}", "  Luego: msc wget cookies <URL> --cdp".dimmed());
                eprintln!();
                eprintln!("{}", "Opción 3: Usar Firefox (sin esta limitación)".green());
                eprintln!("{}", "  msc wget cookies <URL> --browser firefox".dimmed());
            }

            ChromeState::NotRunning if use_cdp => {
                eprintln!(
                    "{}",
                    "Solicitaste CDP pero Chrome no está ejecutándose.".yellow()
                );
                eprintln!();
                eprintln!("{}", "Usa --auto-launch para que msc lance Chrome:".green());
                eprintln!("{}", "  msc wget cookies <URL> --auto-launch".cyan());
            }

            _ => {
                if error.to_string().contains("App-Bound") {
                    eprintln!(
                        "{}",
                        "Chrome 127+ detectado (App-Bound Encryption)".yellow()
                    );
                    eprintln!();
                    eprintln!("{}", "Este error requiere CDP. Prueba:".green());
                    eprintln!("{}", "  1. Cerrar Chrome completamente".dimmed());
                    eprintln!("{}", "  2. msc wget cookies <URL> --auto-launch".cyan());
                } else {
                    eprintln!("{}", "Prueba estas alternativas:".green());
                    eprintln!("{}", "  1. Cerrar Chrome y usar --auto-launch".dimmed());
                    eprintln!("{}", "  2. Usar Firefox: --browser firefox".dimmed());
                }
            }
        }

        eprintln!();
        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".red());
        eprintln!();

        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrome_state_detection() {
        let db_path = PathBuf::from("test.db");
        let manager = ChromeManager::new("chrome", db_path);

        // Should not panic — just detect whatever state Chrome is in
        let _state = manager.detect_chrome_state();
    }

    #[test]
    fn test_strategy_no_flags_gives_direct_database() {
        let db_path = PathBuf::from("test.db");
        let manager = ChromeManager::new("chrome", db_path);

        let (strategy, _) = manager.determine_strategy(false, false);
        // Without flags and without CDP running, should default to DirectDatabase
        // (unless Chrome happens to be running with CDP, which is unlikely in tests)
        if !ChromeManager::is_cdp_available_sync() {
            assert_eq!(strategy, ExtractionStrategy::DirectDatabase);
        }
    }

    #[test]
    fn test_strategy_auto_launch_without_cdp() {
        let db_path = PathBuf::from("test.db");
        let manager = ChromeManager::new("chrome", db_path);

        let (strategy, _) = manager.determine_strategy(false, true);
        // With auto_launch and no existing CDP, should use IsolatedCDP
        if !ChromeManager::is_cdp_available_sync() {
            assert_eq!(strategy, ExtractionStrategy::IsolatedCDP);
        }
    }
}
