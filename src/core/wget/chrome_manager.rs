// Chrome Manager - Intelligent Chrome state detection and cookie extraction orchestration
//
// This module provides a robust, multi-strategy approach to extracting Chrome cookies:
// 1. CDP from existing Chrome instance (if running with debugging)
// 2. Auto-launch Chrome with temporary profile + cookie sync
// 3. Direct database extraction with DPAPI (pre-Chrome 127)
// 4. Graceful failure with clear user instructions

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;
use std::time::Duration;
use sysinfo::{System, ProcessRefreshKind};

use super::chrome_launcher::{
    ChromeInstance,
    kill_all_chrome_processes,
    wait_for_file_release,
    launch_with_original_profile,
    get_original_profile_path,
};
use super::cdp_cookies;
use super::wget_cookies::{extract_cookies_from_db, Cookie};

const CDP_PORT: u16 = 9222;

/// Chrome process state detection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChromeState {
    /// Chrome is not running
    NotRunning,
    /// Chrome is running with CDP enabled on port 9222
    RunningWithCDP,
    /// Chrome is running but without CDP
    RunningWithoutCDP,
}

/// Extraction strategy to use
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtractionStrategy {
    /// Use existing CDP connection
    ExistingCDP,
    /// Restart Chrome with CDP using ORIGINAL profile (NEW - Restart-and-Attach)
    /// This is the core of the new architecture for Chrome 127+
    RestartWithCDP,
    /// Launch Chrome with ORIGINAL profile (MODIFIED - was TempProfile)
    LaunchWithOriginalProfile,
    /// Direct database extraction (fallback for Chrome < 127)
    DirectDatabase,
}

/// Chrome Manager - orchestrates cookie extraction with intelligent fallback
pub struct ChromeManager {
    browser_type: String,
    db_path: PathBuf,
}

impl ChromeManager {
    /// Create a new ChromeManager
    pub fn new(browser: &str, db_path: PathBuf) -> Self {
        Self {
            browser_type: browser.to_string(),
            db_path,
        }
    }

    /// Detect current Chrome state
    pub fn detect_chrome_state(&self) -> ChromeState {
        // First check if CDP port is open
        if Self::is_cdp_available_sync() {
            return ChromeState::RunningWithCDP;
        }

        // Check if Chrome process is running
        let mut system = System::new();
        system.refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());

        let chrome_names = match self.browser_type.as_str() {
            "edge" => vec!["msedge.exe", "msedge"],
            _ => vec!["chrome.exe", "chrome", "google-chrome", "google-chrome-stable"],
        };

        for (_pid, process) in system.processes() {
            let process_name = process.name().to_string_lossy().to_ascii_lowercase();
            if chrome_names.iter().any(|name| process_name.contains(name)) {
                return ChromeState::RunningWithoutCDP;
            }
        }

        ChromeState::NotRunning
    }

    /// Check if CDP is available (synchronous version)
    fn is_cdp_available_sync() -> bool {
        std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", CDP_PORT).parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok()
    }

    /// Determine the best extraction strategy based on flags and Chrome state
    ///
    /// # NEW LOGIC (Restart-and-Attach Architecture)
    /// - If Chrome is running WITHOUT CDP → RestartWithCDP (closes and relaunches)
    /// - If Chrome is NOT running → LaunchWithOriginalProfile
    /// - If Chrome is running WITH CDP → ExistingCDP (use it directly)
    /// - Fallback: DirectDatabase (only works on Chrome < 127 without ABE)
    pub fn determine_strategy(
        &self,
        use_cdp: bool,
        auto_launch: bool,
    ) -> (ExtractionStrategy, ChromeState) {
        let state = self.detect_chrome_state();

        let strategy = match (use_cdp, auto_launch, state) {
            // CDP available → use it directly
            (_, _, ChromeState::RunningWithCDP) => ExtractionStrategy::ExistingCDP,

            // Chrome running WITHOUT CDP + auto-launch → RESTART (CRITICAL CHANGE)
            // This is the core fix for Chrome 127+ ABE
            (_, true, ChromeState::RunningWithoutCDP) => {
                ExtractionStrategy::RestartWithCDP
            }

            // Chrome NOT running + auto-launch → Launch with original profile
            (_, true, ChromeState::NotRunning) => {
                ExtractionStrategy::LaunchWithOriginalProfile
            }

            // User wants CDP but Chrome is not running with it
            (true, false, _) => ExtractionStrategy::ExistingCDP,

            // Fallback: Direct database (only works on Chrome < 127 without ABE)
            _ => ExtractionStrategy::DirectDatabase,
        };

        (strategy, state)
    }

    /// Extract cookies using the best available strategy with automatic fallback
    ///
    /// # Strategy Priority (with fallback):
    /// 1. CDP from existing Chrome (if available)
    /// 2. Auto-launch Chrome with temp profile (if requested)
    /// 3. Direct database extraction (if Chrome not running)
    /// 4. Error with clear instructions
    pub async fn extract_cookies_smart(
        &self,
        domain: &str,
        use_cdp: bool,
        auto_launch: bool,
    ) -> Result<Vec<Cookie>> {
        let (strategy, chrome_state) = self.determine_strategy(use_cdp, auto_launch);

        println!();
        println!("{}", "🔍 Analizando estado de Chrome...".cyan());

        // Report Chrome state
        match chrome_state {
            ChromeState::NotRunning => {
                println!("{}", "   • Chrome no está ejecutándose".dimmed());
            }
            ChromeState::RunningWithCDP => {
                println!("{}", "   • Chrome ejecutándose con CDP habilitado ✓".green());
            }
            ChromeState::RunningWithoutCDP => {
                println!(
                    "{}",
                    "   • Chrome ejecutándose (sin CDP)".yellow()
                );

                // Provide context-specific advice
                if auto_launch {
                    println!(
                        "{}",
                        "   • Usando extracción directa (más rápido y confiable)".green()
                    );
                }
            }
        }

        // Report selected strategy
        let (strategy_name, strategy_desc) = match strategy {
            ExtractionStrategy::ExistingCDP => (
                "Conexión a CDP existente",
                "Usando Chrome que ya está corriendo con CDP"
            ),
            ExtractionStrategy::RestartWithCDP => (
                "Reiniciar Chrome con CDP",
                "Se cerrará Chrome y se relanzará con perfil original + CDP (Restart-and-Attach)"
            ),
            ExtractionStrategy::LaunchWithOriginalProfile => (
                "Lanzar Chrome con perfil original",
                "Se lanzará Chrome con CDP usando tu perfil real (sin copias)"
            ),
            ExtractionStrategy::DirectDatabase => (
                "Extracción directa de base de datos",
                if chrome_state == ChromeState::RunningWithoutCDP {
                    "Leyendo cookies directamente del disco (puede fallar en Chrome 127+ con ABE)"
                } else {
                    "Leyendo cookies directamente del disco"
                }
            ),
        };
        println!("{} {}", "   • Estrategia:".cyan(), strategy_name.bold());
        println!("{}", format!("     {}", strategy_desc).dimmed());
        println!();

        // Execute strategy with fallback
        let result = self
            .execute_strategy_with_fallback(domain, strategy, chrome_state)
            .await;

        // Handle result
        match result {
            Ok(cookies) if !cookies.is_empty() => Ok(cookies),
            Ok(_) => {
                // Empty cookies - not an error, just no cookies for domain
                Ok(vec![])
            }
            Err(e) => {
                // All strategies failed - provide helpful error message
                self.provide_helpful_error(e, chrome_state, use_cdp, auto_launch)
            }
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
                // Try CDP first
                match self.try_cdp_extraction(domain).await {
                    Ok(cookies) => return Ok(cookies),
                    Err(e) => {
                        eprintln!(
                            "{}",
                            format!("⚠️  CDP falló: {}", e).yellow()
                        );
                        eprintln!("{}", "   Intentando método alternativo...".dimmed());
                    }
                }

                // Fallback to database if Chrome is running without CDP
                if chrome_state == ChromeState::RunningWithoutCDP {
                    eprintln!(
                        "{}",
                        "   Chrome está abierto. Algunas cookies recientes pueden no estar en disco.".yellow()
                    );
                }

                self.try_database_extraction(domain)
            }

            ExtractionStrategy::RestartWithCDP => {
                // Try restart-and-attach (NEW STRATEGY)
                match self.execute_restart_with_cdp(domain).await {
                    Ok(cookies) => return Ok(cookies),
                    Err(e) => {
                        eprintln!(
                            "{}",
                            format!("⚠️  Restart-and-Attach falló: {}", e).yellow()
                        );
                        eprintln!("{}", "   Intentando método alternativo...".dimmed());
                    }
                }

                // Fallback to direct database (may fail with ABE)
                self.try_database_extraction(domain)
            }

            ExtractionStrategy::LaunchWithOriginalProfile => {
                // Try launch with original profile (MODIFIED STRATEGY)
                match self.try_launch_with_original_profile(domain).await {
                    Ok(cookies) => return Ok(cookies),
                    Err(e) => {
                        eprintln!(
                            "{}",
                            format!("⚠️  Launch con perfil original falló: {}", e).yellow()
                        );
                        eprintln!("{}", "   Intentando método alternativo...".dimmed());
                    }
                }

                // Fallback to direct database
                self.try_database_extraction(domain)
            }

            ExtractionStrategy::DirectDatabase => {
                // Direct database extraction
                self.try_database_extraction(domain)
            }
        }
    }

    /// Try CDP extraction with retries
    async fn try_cdp_extraction(&self, domain: &str) -> Result<Vec<Cookie>> {
        println!("{}", "⟳ Extrayendo cookies via CDP...".cyan());

        // Check if CDP is available first
        if !cdp_cookies::is_cdp_available().await {
            return Err(anyhow::anyhow!(
                "CDP no está disponible en puerto {}",
                CDP_PORT
            ));
        }

        // Extract with retries
        cdp_cookies::extract_cookies_cdp_with_retry(domain, 3).await
    }

    /// Execute "Restart-and-Attach" strategy
    ///
    /// This is the core of the new architecture for Chrome 127+ ABE support.
    /// Instead of copying the profile (which breaks ABE path binding), we:
    /// 1. Kill all Chrome processes
    /// 2. Wait for file release
    /// 3. Launch Chrome with ORIGINAL profile + CDP
    /// 4. Extract cookies via Storage API
    ///
    /// # Why this works
    /// - Chrome operates on its original profile path
    /// - ABE path binding is preserved
    /// - Chrome can decrypt cookies internally
    /// - CDP returns plaintext cookies from memory
    async fn execute_restart_with_cdp(&self, domain: &str) -> Result<Vec<Cookie>> {
        println!();
        println!("{}", "🔄 Chrome está abierto sin CDP. Reiniciando...".yellow().bold());
        println!();

        // 1. Warning to user
        println!("{}", "   ⚠️  Se cerrarán todas las pestañas de Chrome".yellow());
        println!("{}", "   ⏳ Esperando 3 segundos (Ctrl+C para cancelar)...".dimmed());
        println!();

        tokio::time::sleep(Duration::from_secs(3)).await;

        // 2. Kill Chrome processes
        kill_all_chrome_processes(&self.browser_type)?;
        println!();

        // 3. Wait for file release
        wait_for_file_release(&self.db_path)?;
        println!();

        // 4. Launch Chrome with ORIGINAL profile
        let original_profile = get_original_profile_path(&self.browser_type)?;
        let _chrome_process = launch_with_original_profile(
            &self.browser_type,
            &original_profile,
        )?;

        println!();
        println!("{}", "   ⏳ Esperando que Chrome cargue cookies en memoria...".dimmed());

        // Give Chrome time to initialize CDP and load cookies
        for _i in 1..=5 {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            print!(".");
            use std::io::{self, Write};
            io::stdout().flush().unwrap();
        }
        println!();
        println!();

        // 5. Extract via Storage API
        self.try_cdp_extraction(domain).await
    }

    /// Try launch with ORIGINAL profile (NOT temporary)
    ///
    /// This is the NEW approach that preserves ABE path binding.
    /// Instead of copying the profile, we launch Chrome with the original profile.
    async fn try_launch_with_original_profile(
        &self,
        domain: &str,
    ) -> Result<Vec<Cookie>> {
        println!();
        println!("{}", "🚀 Lanzando Chrome con perfil original...".cyan().bold());
        println!("{}", "   (Sin copias - preserva ABE path binding)".dimmed());
        println!();

        // Launch Chrome with ORIGINAL profile
        let original_profile = get_original_profile_path(&self.browser_type)?;
        let _chrome_process = launch_with_original_profile(
            &self.browser_type,
            &original_profile,
        )?;

        // Give Chrome time to load cookies into memory
        println!();
        println!("{}", "   ⏳ Esperando que Chrome cargue las cookies en memoria...".dimmed());

        // Wait progressively with feedback
        for _i in 1..=5 {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            print!(".");
            use std::io::{self, Write};
            io::stdout().flush().unwrap();
        }
        println!();
        println!();

        // Extract cookies via CDP (using Storage API)
        self.try_cdp_extraction(domain).await
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

        // Provide context-specific suggestions
        eprintln!("{}", "💡 SOLUCIONES SUGERIDAS:".cyan().bold());
        eprintln!();

        match chrome_state {
            ChromeState::RunningWithoutCDP if use_cdp || auto_launch => {
                eprintln!("{}", "Chrome está abierto sin CDP.".yellow());
                eprintln!();
                eprintln!("{}", "Opción 1: Cerrar Chrome completamente".green());
                eprintln!("{}", "  1. Abre el Administrador de Tareas (Ctrl+Shift+Esc)".dimmed());
                eprintln!("{}", "  2. Busca 'chrome.exe' en la pestaña Detalles".dimmed());
                eprintln!("{}", "  3. Finaliza TODOS los procesos de Chrome".dimmed());
                eprintln!("{}", "  4. Vuelve a ejecutar el comando".dimmed());
                eprintln!();
                eprintln!("{}", "Opción 2: Iniciar Chrome manualmente con CDP".green());
                eprintln!(
                    "{}",
                    "  chrome.exe --remote-debugging-port=9222".cyan()
                );
                eprintln!(
                    "{}",
                    "  Luego: msc wget cookies <URL> --cdp".dimmed()
                );
            }

            ChromeState::NotRunning if use_cdp => {
                eprintln!(
                    "{}",
                    "Solicitaste CDP pero Chrome no está ejecutándose.".yellow()
                );
                eprintln!();
                eprintln!("{}", "Opciones:".green());
                eprintln!(
                    "{}",
                    "  1. Usa --auto-launch en lugar de --cdp".cyan()
                );
                eprintln!(
                    "{}",
                    "  2. Inicia Chrome manualmente con:".dimmed()
                );
                eprintln!(
                    "{}",
                    "     chrome.exe --remote-debugging-port=9222".cyan()
                );
            }

            _ => {
                // Check if it's a Chrome 127+ issue
                if error.to_string().contains("App-Bound") {
                    eprintln!("{}", "Chrome 127+ detectado (App-Bound Encryption)".yellow());
                    eprintln!();
                    eprintln!("{}", "Este error requiere CDP. Prueba:".green());
                    eprintln!("{}", "  1. Cerrar Chrome completamente".dimmed());
                    eprintln!(
                        "{}",
                        "  2. msc wget cookies <URL> --auto-launch".cyan()
                    );
                    eprintln!();
                    eprintln!("{}", "Alternativas:".green());
                    eprintln!("{}", "  • Usar Firefox: --browser firefox".dimmed());
                } else {
                    eprintln!("{}", "Prueba estas alternativas:".green());
                    eprintln!("{}", "  1. Cerrar Chrome y usar --auto-launch".dimmed());
                    eprintln!("{}", "  2. Usar Firefox: --browser firefox".dimmed());
                    eprintln!("{}", "  3. Exportar cookies con una extensión".dimmed());
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

        // Just verify it doesn't panic
        let _state = manager.detect_chrome_state();
    }

    #[test]
    fn test_strategy_determination() {
        let db_path = PathBuf::from("test.db");
        let manager = ChromeManager::new("chrome", db_path);

        // Test different scenarios
        let (strategy, _) = manager.determine_strategy(false, false);
        assert_eq!(strategy, ExtractionStrategy::DirectDatabase);
    }
}
