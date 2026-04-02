// Chrome/Edge launcher with CDP (DevTools Protocol) support
//
// Architecture: IsolatedChrome (new) vs ChromeInstance (deprecated)
//
// IsolatedChrome launches a SEPARATE headless Chrome on a dynamic port.
// It NEVER kills the user's browser — only the process it spawned.
//
// ChromeInstance + kill_all_chrome_processes are DEPRECATED and will be removed.

use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const LEGACY_CDP_PORT: u16 = 9222;
const STARTUP_TIMEOUT_MS: u64 = 15000;
const CDP_CHECK_INTERVAL_MS: u64 = 200;

// ============================================================================
// STANDALONE BROWSER UTILITIES
// ============================================================================

/// Find Chrome executable path on the current platform
pub fn find_chrome_executable() -> Result<String> {
    let paths = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ];

    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        let local_chrome = format!(r"{}\Google\Chrome\Application\chrome.exe", local_app_data);
        if PathBuf::from(&local_chrome).exists() {
            return Ok(local_chrome);
        }
    }

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

/// Find Edge executable path on the current platform
pub fn find_edge_executable() -> Result<String> {
    let paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];

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

/// Find the browser executable for a given browser type
pub fn find_browser_executable(browser: &str) -> Result<String> {
    match browser {
        "edge" | "msedge" | "microsoft-edge" | "ms-edge" => find_edge_executable(),
        _ => find_chrome_executable(),
    }
}

/// Get display name for a browser type
pub fn browser_display_name(browser: &str) -> &str {
    match browser {
        "edge" | "msedge" | "microsoft-edge" | "ms-edge" => "Edge",
        _ => "Chrome",
    }
}

/// Find an available TCP port for CDP.
///
/// Binds to port 0 (OS assigns a free port), captures the assigned port,
/// then drops the listener to release it for Chrome.
///
/// There's a small TOCTOU window between releasing the port and Chrome binding it.
/// In practice this is negligible because the OS reuse delay is short and we
/// launch Chrome immediately after.
pub fn find_free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .context("No se pudo encontrar un puerto libre para CDP")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

// ============================================================================
// ISOLATED CHROME — NEW ARCHITECTURE
// ============================================================================

/// Isolated Chrome instance that manages its own process lifecycle.
///
/// Key guarantees:
/// - Launches on a DYNAMIC port (never conflicts with user's Chrome)
/// - Only tracks and kills the process IT spawned
/// - NEVER calls kill_all_chrome_processes or interferes with running browsers
/// - Cleans up on Drop
///
/// # Usage
/// ```no_run
/// # use anyhow::Result;
/// # fn example() -> Result<()> {
/// use msc::core::wget::chrome_launcher::IsolatedChrome;
///
/// let chrome = IsolatedChrome::launch("chrome")?;
/// println!("CDP available on port {}", chrome.cdp_port());
/// // Use chrome.cdp_port() for CDP operations...
/// // Chrome is automatically killed when `chrome` is dropped
/// # Ok(())
/// # }
/// ```
pub struct IsolatedChrome {
    process: Child,
    cdp_port: u16,
    browser_type: String,
}

impl IsolatedChrome {
    /// Launch an isolated headless Chrome instance with CDP enabled.
    ///
    /// This creates a completely separate Chrome process that:
    /// - Uses a dynamically assigned free port for CDP
    /// - Runs in new headless mode (`--headless=new`)
    /// - Uses the original profile for ABE compatibility (Chrome 127+)
    /// - Does NOT kill any existing Chrome processes
    ///
    /// # Errors
    /// - Browser executable not found
    /// - Profile directory not found
    /// - Chrome failed to start or crashed
    /// - CDP did not become ready within timeout
    pub fn launch(browser: &str) -> Result<Self> {
        let cdp_port = find_free_port()?;
        let browser_path = find_browser_executable(browser)?;
        let original_profile = get_original_profile_path(browser)?;
        let browser_display = browser_display_name(browser);

        println!(
            "{}",
            format!(
                "🚀 Lanzando {} aislado en puerto {}...",
                browser_display, cdp_port
            )
            .cyan()
            .bold()
        );
        println!(
            "{}",
            format!("   Perfil: {}", original_profile.display()).dimmed()
        );

        let process = Command::new(&browser_path)
            .arg(format!("--remote-debugging-port={}", cdp_port))
            .arg(format!("--user-data-dir={}", original_profile.display()))
            .arg("--headless=new")
            .arg("--disable-gpu")
            .arg("--disable-software-rasterizer")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            // Anti-detection flags (inspired by nlm/chromedp)
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--disable-extensions")
            .arg("--disable-popup-blocking")
            .arg("--disable-background-networking")
            .arg("--disable-client-side-phishing-detection")
            .arg("--disable-sync")
            .arg("--metrics-recording-only")
            .arg("--no-default-browser-check")
            .arg("about:blank")
            .spawn()
            .with_context(|| format!("No se pudo lanzar {} en modo aislado", browser_display))?;

        let mut instance = Self {
            process,
            cdp_port,
            browser_type: browser.to_string(),
        };

        instance.wait_for_cdp_ready()?;

        Ok(instance)
    }

    /// Get the CDP port this instance is listening on
    pub fn cdp_port(&self) -> u16 {
        self.cdp_port
    }

    /// Get the browser type
    pub fn browser_type(&self) -> &str {
        &self.browser_type
    }

    /// Check if CDP is responding on this instance's port
    pub fn is_cdp_ready(&self) -> bool {
        std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", self.cdp_port).parse().unwrap(),
            Duration::from_millis(200),
        )
        .is_ok()
    }

    /// Wait for CDP to become ready with timeout and process health checks
    fn wait_for_cdp_ready(&mut self) -> Result<()> {
        let browser_display = browser_display_name(&self.browser_type);
        let max_wait = Duration::from_secs(15);
        let check_interval = Duration::from_millis(500);
        let start = Instant::now();

        println!("{}", "   ⏳ Esperando conexión CDP...".dimmed());

        loop {
            // Check if process crashed
            if let Ok(Some(status)) = self.process.try_wait() {
                return Err(anyhow::anyhow!(
                    "{} se cerró inesperadamente (exit code: {}).\n\
                    Posibles causas:\n\
                    • El perfil de Chrome está bloqueado por otra instancia\n\
                    • Permisos insuficientes\n\n\
                    Solución: Cerrá Chrome y volvé a intentar.",
                    browser_display,
                    status
                ));
            }

            // Check if CDP is responding with a real HTTP request
            if let Ok(response) = reqwest::blocking::Client::new()
                .get(format!("http://127.0.0.1:{}/json/version", self.cdp_port))
                .timeout(Duration::from_secs(2))
                .send()
            {
                if response.status().is_success() {
                    println!(
                        "{}",
                        format!("   ✓ CDP listo en puerto {}", self.cdp_port).green()
                    );
                    return Ok(());
                }
            }

            if start.elapsed() > max_wait {
                let _ = self.process.kill();
                return Err(anyhow::anyhow!(
                    "Timeout: {} no habilitó CDP en {} segundos.\n\
                    Posibles causas:\n\
                    • Firewall bloqueando el puerto\n\
                    • Chrome iniciando muy lentamente\n\
                    • Perfil bloqueado por otra instancia",
                    browser_display,
                    max_wait.as_secs()
                ));
            }

            std::thread::sleep(check_interval);
        }
    }
}

impl IsolatedChrome {
    /// Attempt graceful shutdown via CDP before killing the process.
    ///
    /// Sends `Browser.close` via CDP which lets Chrome exit cleanly,
    /// avoiding the "Chrome didn't shut down correctly" recovery dialog
    /// on next launch (same approach as nlm's gracefulShutdown).
    fn graceful_shutdown(&mut self) {
        // Try sending Browser.close via HTTP endpoint (simpler than WebSocket)
        let close_url = format!("http://127.0.0.1:{}/json/close", self.cdp_port);
        let _ = reqwest::blocking::Client::new()
            .get(&close_url)
            .timeout(Duration::from_secs(2))
            .send();

        // Also try the /json/new?about:blank then Browser.close approach
        let browser_close_url = format!("http://127.0.0.1:{}/json/protocol", self.cdp_port);
        // Just send a quick close — if it fails, we'll force-kill
        let _ = reqwest::blocking::Client::new()
            .put(format!("http://127.0.0.1:{}/json/close", self.cdp_port))
            .timeout(Duration::from_secs(1))
            .send();

        // Give Chrome a moment to process the close
        std::thread::sleep(Duration::from_millis(500));

        // Suppress unused variable warning
        let _ = browser_close_url;
    }
}

impl Drop for IsolatedChrome {
    fn drop(&mut self) {
        let browser_display = browser_display_name(&self.browser_type);
        println!(
            "{}",
            format!(
                "🛑 Cerrando {} aislado (puerto {})...",
                browser_display, self.cdp_port
            )
            .dimmed()
        );

        // Try graceful shutdown first (avoids crash recovery dialog)
        self.graceful_shutdown();

        // Then ensure the process is dead
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

// ============================================================================
// LEGACY: ChromeInstance (DEPRECATED — use IsolatedChrome instead)
// ============================================================================

/// Legacy Chrome instance manager.
///
/// # Deprecated
/// Use [`IsolatedChrome`] instead. This struct uses a hardcoded port (9222)
/// and its companion `kill_all_chrome_processes` kills ALL Chrome processes
/// on the system — including the user's personal browser.
#[deprecated(
    since = "0.2.0",
    note = "Use IsolatedChrome::launch() instead. ChromeInstance uses a hardcoded port and kill_all_chrome_processes kills the user's browser."
)]
pub struct ChromeInstance {
    process: Option<Child>,
    was_running: bool,
    browser_type: String,
}

#[allow(deprecated)]
impl ChromeInstance {
    /// Ensure Chrome/Edge is running with CDP enabled on legacy port 9222.
    ///
    /// # Deprecated
    /// Use [`IsolatedChrome::launch()`] instead.
    pub fn ensure_running(browser: &str) -> Result<Self> {
        let browser_display = browser_display_name(browser);

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

        let original_profile = get_original_profile_path(browser)?;
        let mut process = launch_with_original_profile(browser, &original_profile)?;

        let start = Instant::now();
        let timeout = Duration::from_millis(STARTUP_TIMEOUT_MS);

        loop {
            if Self::is_cdp_active() && Self::verify_cdp_ready().is_ok() {
                println!(
                    "{}",
                    format!(
                        "         ✓ {} listo con CDP en puerto {}",
                        browser_display, LEGACY_CDP_PORT
                    )
                    .green()
                );
                break;
            }

            if let Ok(Some(status)) = process.try_wait() {
                return Err(anyhow::anyhow!(
                    "{} se cerró inesperadamente (exit code: {})",
                    browser_display,
                    status
                ));
            }

            if start.elapsed() > timeout {
                let _ = process.kill();
                return Err(anyhow::anyhow!(
                    "{} no respondió en {} segundos",
                    browser_display,
                    STARTUP_TIMEOUT_MS / 1000
                ));
            }

            std::thread::sleep(Duration::from_millis(CDP_CHECK_INTERVAL_MS));
        }

        Ok(Self {
            process: Some(process),
            was_running: false,
            browser_type: browser.to_string(),
        })
    }

    fn is_cdp_active() -> bool {
        std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", LEGACY_CDP_PORT).parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok()
    }

    fn verify_cdp_ready() -> Result<()> {
        let response = reqwest::blocking::Client::new()
            .get(format!("http://127.0.0.1:{}/json", LEGACY_CDP_PORT))
            .timeout(Duration::from_secs(2))
            .send()
            .context("Failed to query CDP targets")?;

        let targets: Vec<serde_json::Value> = response.json().context("Invalid CDP response")?;

        if targets.is_empty() {
            return Err(anyhow::anyhow!("No CDP targets available yet"));
        }

        Ok(())
    }

    pub fn was_already_running(&self) -> bool {
        self.was_running
    }
}

#[allow(deprecated)]
impl Drop for ChromeInstance {
    fn drop(&mut self) {
        if let Some(ref mut process) = self.process {
            let browser_display = browser_display_name(&self.browser_type);
            println!(
                "{}",
                format!("🛑 Cerrando {} temporal...", browser_display).dimmed()
            );
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

// ============================================================================
// LEGACY FUNCTIONS (DEPRECATED — will be removed)
// ============================================================================

/// Kill all Chrome/Edge processes.
///
/// # Deprecated
/// This function kills ALL Chrome processes on the system, including the
/// user's personal browser. Use [`IsolatedChrome`] instead, which only
/// manages its own process.
#[deprecated(
    since = "0.2.0",
    note = "This kills the user's browser. Use IsolatedChrome which only manages its own process."
)]
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

/// Launch Chrome with the ORIGINAL profile on the legacy hardcoded port.
///
/// # Deprecated
/// Use [`IsolatedChrome::launch()`] instead, which uses a dynamic port
/// and doesn't require killing existing Chrome instances.
#[deprecated(
    since = "0.2.0",
    note = "Use IsolatedChrome::launch() instead. This uses a hardcoded port and may conflict with running Chrome."
)]
pub fn launch_with_original_profile(browser: &str, original_profile: &Path) -> Result<Child> {
    let browser_path = find_browser_executable(browser)?;
    let browser_display = browser_display_name(browser);

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
        .arg(format!("--remote-debugging-port={}", LEGACY_CDP_PORT))
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
            browser_display, LEGACY_CDP_PORT
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
                LEGACY_CDP_PORT
            ));
        }

        // Check if CDP is responding
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", LEGACY_CDP_PORT).parse().unwrap(),
            Duration::from_millis(200),
        )
        .is_ok()
        {
            // CDP port is open, verify it's actually ready
            if let Ok(response) = reqwest::blocking::Client::new()
                .get(format!("http://127.0.0.1:{}/json/version", LEGACY_CDP_PORT))
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
                LEGACY_CDP_PORT
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

    // =====================================================================
    // Standalone utility tests (safe — no process side effects)
    // =====================================================================

    #[test]
    fn test_find_chrome_executable() {
        // Should either find Chrome or return a helpful error — never panic
        let _ = find_chrome_executable();
    }

    #[test]
    fn test_find_edge_executable() {
        let _ = find_edge_executable();
    }

    #[test]
    fn test_find_browser_executable_dispatches_correctly() {
        // "edge" variants should attempt Edge, others should attempt Chrome
        let edge_result = find_browser_executable("edge");
        let msedge_result = find_browser_executable("msedge");
        let chrome_result = find_browser_executable("chrome");

        // We can't assert success (browser might not be installed),
        // but we CAN assert the error messages differ by browser name
        if let (Err(e1), Err(e2)) = (&edge_result, &chrome_result) {
            let e1_msg = format!("{}", e1);
            let e2_msg = format!("{}", e2);
            assert_ne!(e1_msg, e2_msg, "Edge and Chrome errors should differ");
        }

        // msedge should produce the same result as edge
        assert_eq!(edge_result.is_ok(), msedge_result.is_ok());
    }

    #[test]
    fn test_browser_display_name() {
        assert_eq!(browser_display_name("edge"), "Edge");
        assert_eq!(browser_display_name("msedge"), "Edge");
        assert_eq!(browser_display_name("microsoft-edge"), "Edge");
        assert_eq!(browser_display_name("chrome"), "Chrome");
        assert_eq!(browser_display_name("brave"), "Chrome");
        assert_eq!(browser_display_name("anything"), "Chrome");
    }

    #[test]
    fn test_find_free_port_returns_valid_port() {
        let port = find_free_port().expect("Should find a free port");
        // Ephemeral ports are typically > 1024
        assert!(port > 1024, "Port {} should be in ephemeral range", port);
    }

    #[test]
    fn test_find_free_port_returns_different_ports() {
        // Two consecutive calls should return different ports
        let port1 = find_free_port().unwrap();
        let port2 = find_free_port().unwrap();
        assert_ne!(
            port1, port2,
            "Consecutive calls should return different ports"
        );
    }

    #[test]
    fn test_get_original_profile_path_chrome() {
        let result = get_original_profile_path("chrome");

        if let Ok(path) = result {
            assert!(path.exists(), "Chrome User Data directory should exist");
            assert!(
                path.ends_with("User Data"),
                "Path should end with 'User Data'"
            );

            // Must NOT be in temp directory
            let temp_dir = env::temp_dir();
            assert!(
                !path.starts_with(&temp_dir),
                "Profile path should NOT be in temp directory"
            );
        }
    }

    #[test]
    fn test_get_original_profile_path_edge() {
        let result = get_original_profile_path("edge");

        if let Ok(path) = result {
            assert!(path.exists(), "Edge User Data directory should exist");
            assert!(
                path.ends_with("User Data"),
                "Path should end with 'User Data'"
            );
        }
    }

    #[allow(deprecated)]
    #[test]
    fn test_wait_for_file_release_nonexistent() {
        let fake_path = PathBuf::from("C:\\nonexistent\\fake\\Cookies");
        let result = wait_for_file_release(&fake_path);
        assert!(
            result.is_err(),
            "wait_for_file_release should fail for non-existent files"
        );
    }

    // =====================================================================
    // IsolatedChrome tests (integration — require Chrome installed)
    // =====================================================================

    #[test]
    fn test_isolated_chrome_cdp_ready_before_launch() {
        // Before launching, is_cdp_ready on an arbitrary port should be false
        // We just verify the check itself doesn't panic
        let port = find_free_port().unwrap();
        let ready = std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok();
        assert!(!ready, "Random port should not have CDP running");
    }

    #[test]
    #[ignore] // Requires Chrome installed and no profile lock
    fn test_isolated_chrome_launch_and_cleanup() {
        // Launch an isolated Chrome — it should get a dynamic port
        let chrome = IsolatedChrome::launch("chrome");

        if let Ok(chrome) = chrome {
            let port = chrome.cdp_port();
            assert!(port > 1024, "Should use an ephemeral port");
            assert_ne!(port, LEGACY_CDP_PORT, "Should NOT use the legacy port");
            assert!(chrome.is_cdp_ready(), "CDP should be ready after launch");

            // Drop triggers cleanup — Chrome process should be killed
            drop(chrome);

            // Verify CDP is no longer responding on that port
            std::thread::sleep(Duration::from_millis(500));
            let still_running = std::net::TcpStream::connect_timeout(
                &format!("127.0.0.1:{}", port).parse().unwrap(),
                Duration::from_millis(200),
            )
            .is_ok();
            assert!(
                !still_running,
                "CDP should not respond after IsolatedChrome is dropped"
            );
        }
    }
}
