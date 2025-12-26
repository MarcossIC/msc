// Chrome/Edge launcher with CDP (DevTools Protocol) support
// Enhanced version with temporary profile and cookie synchronization

use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const CDP_PORT: u16 = 9222;
const STARTUP_TIMEOUT_MS: u64 = 15000; // Increased to 15 seconds
const CDP_CHECK_INTERVAL_MS: u64 = 200;

/// Chrome/Edge instance manager
///
/// Enhanced launcher that:
/// - Uses a temporary user profile to avoid conflicts
/// - Copies cookies from real profile to temp profile
/// - Provides detailed progress feedback
/// - Handles all edge cases gracefully
pub struct ChromeInstance {
    process: Option<Child>,
    was_running: bool,
    browser_type: String,
}

impl ChromeInstance {
    /// Ensure Chrome/Edge is running with CDP enabled
    ///
    /// # NEW ARCHITECTURE (Restart-and-Attach)
    /// 1. Check if browser is already running with CDP -> use it
    /// 2. If not, launch with ORIGINAL profile + CDP (NO temporary profile)
    /// 3. Wait for CDP to be fully ready
    ///
    /// # Why ORIGINAL profile instead of temporary?
    /// - Chrome 127+ App-Bound Encryption requires original path
    /// - ABE path binding validates profile location
    /// - Copying profile breaks encryption chain
    /// - This is the ONLY way to get decrypted cookies in Chrome 127+
    ///
    /// # DEPRECATED WARNING
    /// This function is kept for backward compatibility but should be replaced
    /// by using ChromeManager with the RestartWithCDP strategy.
    pub fn ensure_running(browser: &str) -> Result<Self> {
        let browser_display = if browser == "edge" { "Edge" } else { "Chrome" };

        // Check if already running with CDP
        if Self::is_cdp_active() {
            println!(
                "{}",
                format!("✓ {} ya está corriendo con CDP", browser_display).green()
            );
            return Ok(Self {
                process: None,
                was_running: true,
                browser_type: browser.to_string(),
            });
        }

        println!(
            "{}",
            format!("🚀 Configurando {} con CDP...", browser_display)
                .cyan()
                .bold()
        );
        println!();

        // Get original profile path
        println!(
            "{}",
            "   [1/3] Obteniendo ruta del perfil original...".dimmed()
        );
        let original_profile = get_original_profile_path(browser)?;
        println!(
            "{}",
            format!("         ✓ Perfil: {}", original_profile.display()).dimmed()
        );

        // Launch browser with ORIGINAL profile
        println!(
            "{}",
            "   [2/3] Iniciando navegador con perfil original...".dimmed()
        );
        let mut process = launch_with_original_profile(browser, &original_profile)?;
        println!(
            "{}",
            format!("         ✓ {} iniciado", browser_display).dimmed()
        );

        // Wait for CDP with progress feedback
        println!("{}", "   [3/3] Esperando conexión CDP...".dimmed());
        let start = Instant::now();
        let timeout = Duration::from_millis(STARTUP_TIMEOUT_MS);
        let mut last_dot_time = Instant::now();
        let mut dots = 0;

        loop {
            // Check if CDP is ready
            if Self::is_cdp_active() {
                // Verify we can actually get targets
                if Self::verify_cdp_ready().is_ok() {
                    println!();
                    println!(
                        "{}",
                        format!(
                            "         ✓ {} listo con CDP en puerto {}",
                            browser_display, CDP_PORT
                        )
                        .green()
                    );
                    break;
                }
            }

            // Check if process crashed
            if let Ok(Some(status)) = process.try_wait() {
                return Err(anyhow::anyhow!(
                    "{} se cerró inesperadamente (exit code: {}).\n\n\
                    Causas posibles:\n\
                    • Puerto {} ya está en uso\n\
                    • Permisos insuficientes\n\
                    • Perfil de Chrome ya está en uso por otra instancia\n\n\
                    Solución:\n\
                    1. Cierra TODAS las instancias de Chrome\n\
                    2. Verifica que el puerto {} esté libre\n\
                    3. Vuelve a intentar",
                    browser_display,
                    status,
                    CDP_PORT,
                    CDP_PORT
                ));
            }

            // Check timeout
            if start.elapsed() > timeout {
                let _ = process.kill();

                return Err(anyhow::anyhow!(
                    "{} no respondió en {} segundos.\n\n\
                    Posibles causas:\n\
                    • Firewall bloqueando puerto {}\n\
                    • {} iniciando muy lentamente\n\
                    • Otra instancia ya tiene el perfil abierto\n\n\
                    Solución:\n\
                    • Cierra TODAS las instancias de Chrome\n\
                    • Verifica configuración de firewall",
                    browser_display,
                    STARTUP_TIMEOUT_MS / 1000,
                    CDP_PORT,
                    browser_display
                ));
            }

            // Visual progress feedback (dots)
            if last_dot_time.elapsed() > Duration::from_millis(500) {
                print!(".");
                use std::io::{self, Write};
                io::stdout().flush().unwrap();
                dots += 1;
                if dots > 20 {
                    print!("\r         ");
                    io::stdout().flush().unwrap();
                    dots = 0;
                }
                last_dot_time = Instant::now();
            }

            std::thread::sleep(Duration::from_millis(CDP_CHECK_INTERVAL_MS));
        }

        println!();

        Ok(Self {
            process: Some(process),
            was_running: false,
            browser_type: browser.to_string(),
        })
    }

    // ============================================================================
    // REMOVED OBSOLETE FUNCTIONS (Restart-and-Attach Architecture)
    // ============================================================================
    //
    // The following functions were removed because they implement the OLD
    // "temporary profile" approach that FAILS with Chrome 127+ ABE:
    //
    // - create_temp_profile() - Created temporary profile in %TEMP%
    // - sync_cookies_to_temp_profile() - Copied profile (breaks ABE path binding)
    // - copy_dir_recursive() - Helper for profile copying
    // - get_real_profile_path() - Replaced by get_original_profile_path()
    //
    // These functions are now OBSOLETE. Use the new architecture:
    // - get_original_profile_path() - Gets original User Data directory
    // - launch_with_original_profile() - Launches with original profile
    // - kill_all_chrome_processes() - Kills Chrome when needed
    // - wait_for_file_release() - Waits for file unlock
    // ============================================================================

    /// Check if CDP port is responding
    fn is_cdp_active() -> bool {
        std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", CDP_PORT).parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok()
    }

    /// Verify CDP is fully ready by checking for targets
    fn verify_cdp_ready() -> Result<()> {
        let response = reqwest::blocking::Client::new()
            .get(format!("http://127.0.0.1:{}/json", CDP_PORT))
            .timeout(Duration::from_secs(2))
            .send()
            .context("Failed to query CDP targets")?;

        let targets: Vec<serde_json::Value> = response.json().context("Invalid CDP response")?;

        if targets.is_empty() {
            return Err(anyhow::anyhow!("No CDP targets available yet"));
        }

        Ok(())
    }

    /// Find Chrome executable path
    fn find_chrome_executable() -> Result<String> {
        let paths = [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];

        // Check %LOCALAPPDATA%
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            let local_chrome = format!(r"{}\Google\Chrome\Application\chrome.exe", local_app_data);
            if PathBuf::from(&local_chrome).exists() {
                return Ok(local_chrome);
            }
        }

        // Check standard paths
        paths
            .iter()
            .find(|p| PathBuf::from(p).exists())
            .map(|s| s.to_string())
            .context(
                "Chrome no encontrado.\n\
                Rutas buscadas:\n\
                - C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe\n\
                - C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe\n\
                - %LOCALAPPDATA%\\Google\\Chrome\\Application\\chrome.exe",
            )
    }

    /// Find Edge executable path
    fn find_edge_executable() -> Result<String> {
        let paths = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        ];

        // Check %PROGRAMFILES(X86)%
        if let Ok(program_files_x86) = env::var("ProgramFiles(x86)") {
            let edge_path = format!(
                r"{}\Microsoft\Edge\Application\msedge.exe",
                program_files_x86
            );
            if PathBuf::from(&edge_path).exists() {
                return Ok(edge_path);
            }
        }

        paths
            .iter()
            .find(|p| PathBuf::from(p).exists())
            .map(|s| s.to_string())
            .context(
                "Edge no encontrado.\n\
                Rutas buscadas:\n\
                - C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe\n\
                - C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
            )
    }

    /// Check if this instance was already running
    pub fn was_already_running(&self) -> bool {
        self.was_running
    }
}

impl Drop for ChromeInstance {
    fn drop(&mut self) {
        // Kill process if we started it
        if let Some(ref mut process) = self.process {
            let browser_display = if self.browser_type == "edge" {
                "Edge"
            } else {
                "Chrome"
            };
            println!();
            println!(
                "{}",
                format!("🛑 Cerrando {} temporal...", browser_display).dimmed()
            );
            let _ = process.kill();
            let _ = process.wait(); // Wait for process to fully terminate
        }
    }
}

// ============================================================================
// NEW FUNCTIONS FOR "RESTART-AND-ATTACH" ARCHITECTURE
// ============================================================================

/// Kill all Chrome/Edge processes
///
/// This is used when Chrome is running without CDP and we need to restart it.
/// Attempts graceful termination first, then force-kills if needed.
///
/// # Arguments
/// * `browser` - "chrome" or "edge"
///
/// # Returns
/// * `Ok(())` if processes were killed or none were running
/// * `Err(...)` if killing failed
pub fn kill_all_chrome_processes(browser: &str) -> Result<()> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());

    let process_name = if browser == "edge" {
        "msedge.exe"
    } else {
        "chrome.exe"
    };

    let chrome_pids: Vec<_> = system
        .processes()
        .iter()
        .filter(|(_, proc)| {
            proc.name()
                .to_string_lossy()
                .to_lowercase()
                .contains(process_name)
        })
        .map(|(pid, _)| *pid)
        .collect();

    if chrome_pids.is_empty() {
        println!("{}", "   ℹ️  No hay procesos de Chrome corriendo".dimmed());
        return Ok(());
    }

    println!(
        "{}",
        format!("   🔫 Cerrando {} procesos de Chrome...", chrome_pids.len()).yellow()
    );

    // Attempt graceful termination
    for pid in &chrome_pids {
        if let Some(process) = system.process(*pid) {
            process.kill();
        }
    }

    // Wait for processes to terminate
    std::thread::sleep(Duration::from_secs(2));

    // Check if any processes remain
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let remaining: Vec<_> = system
        .processes()
        .iter()
        .filter(|(_, proc)| {
            proc.name()
                .to_string_lossy()
                .to_lowercase()
                .contains(process_name)
        })
        .collect();

    if !remaining.is_empty() {
        println!(
            "{}",
            format!(
                "   ⚠️  {} procesos no respondieron al cierre graceful",
                remaining.len()
            )
            .yellow()
        );

        // On Windows, use taskkill /F as last resort
        #[cfg(windows)]
        {
            println!("{}", "   🔨 Intentando terminación forzada...".yellow());
            let output = std::process::Command::new("taskkill")
                .arg("/F")
                .arg("/IM")
                .arg(process_name)
                .output();

            match output {
                Ok(output) => {
                    if !output.status.success() {
                        println!(
                            "{}",
                            "   ⚠️  taskkill falló. Algunos procesos pueden seguir activos."
                                .yellow()
                        );
                    } else {
                        println!("{}", "   ✓ Procesos terminados forzadamente".green());
                    }
                }
                Err(e) => {
                    println!(
                        "{}",
                        format!("   ⚠️  No se pudo ejecutar taskkill: {}", e).yellow()
                    );
                }
            }
        }
    } else {
        println!("{}", "   ✓ Todos los procesos de Chrome cerrados".green());
    }

    Ok(())
}

/// Wait for Chrome to release file locks on the database
///
/// After killing Chrome, we need to wait for Windows to release file handles.
/// This function polls the database file until it can be opened exclusively.
///
/// # Arguments
/// * `db_path` - Path to the Cookies database file
///
/// # Returns
/// * `Ok(())` if files were released
/// * `Err(...)` if timeout occurred
pub fn wait_for_file_release(db_path: &PathBuf) -> Result<()> {
    use std::fs::OpenOptions;

    println!("{}", "   ⏳ Esperando liberación de archivos...".dimmed());

    let max_attempts = 15; // 15 * 200ms = 3 seconds
    let mut last_error = None;

    for attempt in 0..max_attempts {
        // Try to open with exclusive write access
        // This will fail if Chrome still has the file locked
        match OpenOptions::new().write(true).open(db_path) {
            Ok(_) => {
                println!("{}", "   ✓ Archivos liberados".green());
                return Ok(());
            }
            Err(e) => {
                last_error = Some(e);
                if attempt < max_attempts - 1 {
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                }
            }
        }
    }

    Err(anyhow::anyhow!(
        "Chrome no liberó los archivos después de 3 segundos: {}",
        last_error.unwrap()
    ))
}

/// Launch Chrome with the ORIGINAL profile (not a temporary copy)
///
/// This is the core of the "Restart-and-Attach" architecture.
/// Instead of copying the profile, we launch Chrome with its original profile path.
/// This preserves the ABE encryption chain of trust.
///
/// # Arguments
/// * `browser` - "chrome" or "edge"
/// * `original_profile` - Path to the original "User Data" directory
///
/// # Returns
/// * `Ok(Child)` - The Chrome process handle
/// * `Err(...)` if launch failed
pub fn launch_with_original_profile(browser: &str, original_profile: &PathBuf) -> Result<Child> {
    let browser_path = if browser == "edge" {
        ChromeInstance::find_edge_executable()
    } else {
        ChromeInstance::find_chrome_executable()
    }?;

    let browser_display = if browser == "edge" { "Edge" } else { "Chrome" };

    println!(
        "{}",
        format!("🚀 Lanzando {} con perfil original...", browser_display)
            .cyan()
            .bold()
    );
    println!(
        "{}",
        format!("   Perfil: {}", original_profile.display()).dimmed()
    );

    let mut process = Command::new(&browser_path)
        .arg(format!("--remote-debugging-port={}", CDP_PORT))
        .arg(format!("--user-data-dir={}", original_profile.display()))
        .arg("--headless=new") // New headless mode (supports extensions & sessions)
        .arg("--disable-gpu")
        .arg("--disable-software-rasterizer")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("about:blank")
        .spawn()
        .context("Failed to spawn Chrome with original profile")?;

    println!(
        "{}",
        format!(
            "   ✓ {} iniciado con CDP en puerto {}",
            browser_display, CDP_PORT
        )
        .green()
    );

    // Wait for CDP to be fully ready with active verification
    println!(
        "{}",
        "   ⏳ Esperando a que CDP esté disponible...".dimmed()
    );

    let max_wait_time = Duration::from_secs(15); // Total: 15 segundos
    let check_interval = Duration::from_millis(500);
    let start_time = std::time::Instant::now();

    loop {
        // Check if process is still alive
        if let Ok(Some(status)) = process.try_wait() {
            return Err(anyhow::anyhow!(
                "{} se cerró inesperadamente con código: {}\n\
                Esto puede indicar:\n\
                • Puerto {} ya está en uso\n\
                • Problema con el perfil de usuario\n\
                • Permisos insuficientes",
                browser_display,
                status,
                CDP_PORT
            ));
        }

        // Check if CDP is responding
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", CDP_PORT).parse().unwrap(),
            Duration::from_millis(200),
        )
        .is_ok()
        {
            // CDP port is open, verify it's actually ready
            if let Ok(response) = reqwest::blocking::Client::new()
                .get(format!("http://127.0.0.1:{}/json/version", CDP_PORT))
                .timeout(Duration::from_secs(2))
                .send()
            {
                if response.status().is_success() {
                    println!("{}", "   ✓ CDP está listo y respondiendo".green());
                    break;
                }
            }
        }

        // Check timeout
        if start_time.elapsed() > max_wait_time {
            let _ = process.kill();
            return Err(anyhow::anyhow!(
                "Timeout: {} no habilitó CDP en {} segundos.\n\
                Posibles causas:\n\
                • Firewall bloqueando puerto {}\n\
                • Chrome iniciando muy lentamente\n\
                • Conflicto con otra instancia",
                browser_display,
                max_wait_time.as_secs(),
                CDP_PORT
            ));
        }

        // Wait before next check
        std::thread::sleep(check_interval);
        print!(".");
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
    }
    println!();

    Ok(process)
}

/// Get the original Chrome "User Data" directory path
///
/// This returns the parent directory of the Default profile,
/// which is what Chrome expects for --user-data-dir.
///
/// # Arguments
/// * `browser` - "chrome" or "edge"
///
/// # Returns
/// * `Ok(PathBuf)` - Path to "User Data" directory
/// * `Err(...)` if not found
pub fn get_original_profile_path(browser: &str) -> Result<PathBuf> {
    let local_app_data =
        env::var("LOCALAPPDATA").context("LOCALAPPDATA environment variable not set")?;

    let base_path = PathBuf::from(local_app_data);

    let user_data_dir = match browser {
        "edge" => base_path.join("Microsoft").join("Edge").join("User Data"),
        _ => base_path.join("Google").join("Chrome").join("User Data"),
    };

    if !user_data_dir.exists() {
        return Err(anyhow::anyhow!(
            "Directorio de usuario de {} no encontrado en: {}",
            browser,
            user_data_dir.display()
        ));
    }

    Ok(user_data_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdp_port_check() {
        // Should not panic, just return true/false
        let _ = ChromeInstance::is_cdp_active();
    }

    #[test]
    fn test_find_chrome() {
        // Should either find Chrome or return a helpful error
        let _ = ChromeInstance::find_chrome_executable();
    }

    #[test]
    fn test_find_edge() {
        // Should either find Edge or return a helpful error
        let _ = ChromeInstance::find_edge_executable();
    }

    #[test]
    fn test_get_original_profile_path() {
        // NEW TEST: Verify we can get the original Chrome profile path
        // This should succeed if Chrome is installed
        let result = get_original_profile_path("chrome");

        // If Chrome is installed, path should exist
        if let Ok(path) = result {
            assert!(path.exists(), "Chrome User Data directory should exist");
            assert!(
                path.ends_with("User Data"),
                "Path should end with 'User Data'"
            );

            // Verify it's NOT a temp directory
            let temp_dir = env::temp_dir();
            assert!(
                !path.starts_with(&temp_dir),
                "Profile path should NOT be in temp directory (old architecture)"
            );
        }
    }

    #[test]
    fn test_get_original_profile_path_edge() {
        // NEW TEST: Verify we can get the original Edge profile path
        let result = get_original_profile_path("edge");

        // If Edge is installed, path should exist
        if let Ok(path) = result {
            assert!(path.exists(), "Edge User Data directory should exist");
            assert!(
                path.ends_with("User Data"),
                "Path should end with 'User Data'"
            );
        }
    }

    #[test]
    fn test_kill_chrome_when_not_running() {
        // NEW TEST: Verify kill function doesn't panic when Chrome isn't running
        let result = kill_all_chrome_processes("chrome");

        // Should succeed even if Chrome isn't running
        assert!(
            result.is_ok(),
            "kill_all_chrome_processes should not fail when Chrome isn't running"
        );
    }

    #[test]
    fn test_wait_for_file_release_nonexistent() {
        // NEW TEST: Verify wait function handles non-existent files gracefully
        let fake_path = PathBuf::from("C:\\nonexistent\\fake\\Cookies");
        let result = wait_for_file_release(&fake_path);

        // Should fail gracefully for non-existent files
        assert!(
            result.is_err(),
            "wait_for_file_release should fail for non-existent files"
        );
    }

    #[test]
    #[ignore] // Ignore by default - requires Chrome to be closed
    fn test_launch_with_original_profile_integration() {
        // INTEGRATION TEST: Actually launch Chrome with original profile
        // This test is ignored by default because it:
        // 1. Requires Chrome to be fully closed
        // 2. Will launch a real Chrome instance
        // 3. May interfere with user's work

        let original_profile =
            get_original_profile_path("chrome").expect("Chrome should be installed for this test");

        let result = launch_with_original_profile("chrome", &original_profile);

        if let Ok(mut process) = result {
            // Give Chrome time to start
            std::thread::sleep(Duration::from_secs(2));

            // Verify CDP is active
            assert!(
                ChromeInstance::is_cdp_active(),
                "CDP should be active after launch"
            );

            // Clean up
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}
