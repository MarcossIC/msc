use crate::core::wget::{
    calculate_local_path_for_url, create_cookie_file, debug_database_info, extract_cookies_from_db,
    find_browser_cookie_db, format_cookies, process_html_file_complete, resolve_cookie_path,
    WgetManager,
};
use crate::core::{validation, Config};
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use dialoguer::Input;
use std::collections::{HashSet, VecDeque};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use url::Url;

/// Execute post-processing on already downloaded files
pub fn execute_postprocessing(matches: &clap::ArgMatches) -> Result<()> {
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!("{}", "  Post-procesamiento de Archivos".cyan().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!();

    // 1. Extract path argument
    let path_str = matches
        .get_one::<String>("path")
        .context("Ruta es requerida")?;

    let target_dir = PathBuf::from(path_str);

    // 2. Validate that the directory exists
    if !target_dir.exists() {
        return Err(anyhow!("La ruta no existe: {}", target_dir.display()));
    }

    if !target_dir.is_dir() {
        return Err(anyhow!(
            "La ruta no es un directorio: {}",
            target_dir.display()
        ));
    }

    println!("{} {}", "📁 Directorio:".cyan(), target_dir.display());

    // 3. Get base URL (optional, but recommended for proper link resolution)
    let base_url = if let Some(url_str) = matches.get_one::<String>("url") {
        println!("{} {}", "🌐 Base URL:".cyan(), url_str);
        Url::parse(url_str).context("URL inválida")?
    } else {
        // Try to infer from directory structure
        // If directory name looks like a domain, use it
        let domain = target_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("example.com");

        let url_msg = format!("https://{}", domain);
        println!(
            "{} {}",
            "⚠️  URL no especificada, usando:".yellow(),
            url_msg
        );
        Url::parse(&format!("https://{}", domain))?
    };

    println!();

    // 4. Run post-processing
    println!("{}", "⟳ Procesando archivos HTML...".cyan());
    post_process_directory(&target_dir, &target_dir, &base_url)?;

    println!();
    println!(
        "{}",
        "✓ Post-procesamiento completado exitosamente"
            .green()
            .bold()
    );
    println!();

    Ok(())
}

/// Extract cookies from browser for a given URL
pub fn execute_cookies(matches: &clap::ArgMatches) -> Result<()> {
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!("{}", "  Extractor de Cookies".cyan().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!();

    // 1. Extract arguments
    let url_str = matches
        .get_one::<String>("url")
        .context("URL es requerida")?;

    let browser = matches
        .get_one::<String>("browser")
        .map(|s| s.as_str())
        .unwrap_or("chrome");

    let format = matches
        .get_one::<String>("format")
        .map(|s| s.as_str())
        .unwrap_or("wget");

    let output_file = matches.get_one::<String>("output");
    let debug_mode = matches.get_flag("debug");

    // 2. Parse URL to get domain
    let url = Url::parse(url_str).context("URL inválida")?;
    let domain = url.domain().context("URL debe tener un dominio")?;

    println!("{} {}", "🌐 URL:".cyan(), url_str);
    println!("{} {}", "🏠 Dominio:".cyan(), domain);
    println!("{} {}", "🌍 Navegador:".cyan(), browser);
    println!("{} {}", "📋 Formato:".cyan(), format);
    if debug_mode {
        println!("{} Activado", "🐛 Modo Debug:".yellow());
    }
    println!();

    // 3. Find browser cookie database
    let cookie_db_path = find_browser_cookie_db(browser)?;
    println!(
        "{} {}",
        "📂 Base de datos:".green(),
        cookie_db_path.display()
    );
    println!();

    // 3.5. Debug mode - show database info
    if debug_mode {
        debug_database_info(&cookie_db_path)?;
        return Ok(());
    }

    // 4. Extract cookies from database
    println!("{}", "⟳ Extrayendo cookies...".cyan());
    let cookies = extract_cookies_from_db(&cookie_db_path, domain)?;

    if cookies.is_empty() {
        println!();
        println!(
            "{}",
            "⚠️  No se encontraron cookies para este dominio.".yellow()
        );
        println!();
        println!("{}", "Posibles razones:".dimmed());
        println!(
            "{}",
            "  • No has visitado este sitio en este navegador".dimmed()
        );
        println!("{}", "  • Las cookies expiraron o fueron borradas".dimmed());
        println!(
            "{}",
            "  • El navegador está cerrado (cierra y vuelve a intentar)".dimmed()
        );
        println!();
        return Ok(());
    }

    println!("{} {} cookies", "✓ Encontradas".green(), cookies.len());
    println!();

    // 5. Format cookies based on requested format
    let output = format_cookies(&cookies, format, domain)?;

    // 6. Output to file or stdout
    if let Some(file_path) = output_file {
        fs::write(file_path, &output)?;
        println!("{} {}", "✓ Cookies guardadas en:".green().bold(), file_path);
    } else {
        println!("{}", "📋 Cookies en formato wget:".cyan().bold());
        println!();
        println!("{}", output);
    }

    println!();
    println!();
    println!("{}", "💡 Uso:".cyan().bold());
    if format == "wget" {
        println!(
            "{}",
            format!("   msc wget \"{}\" --cookies '{}'", url_str, output.trim()).dimmed()
        );
    } else if format == "netscape" {
        if let Some(file) = output_file {
            println!(
                "{}",
                format!("   msc wget \"{}\" --load-cookies {}", url_str, file).dimmed()
            );
        } else {
            println!(
                "{}",
                format!("   msc wget \"{}\" --load-cookies cookies.txt", url_str).dimmed()
            );
            println!(
                "{}",
                "   (Recomendado guardar en archivo para sitios complejos)"
                    .yellow()
                    .dimmed()
            );
        }
    }
    println!();

    Ok(())
}

/// Execute the wget command to download web pages
pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    // 1. Extract arguments
    let url_str = matches
        .get_one::<String>("url")
        .context("URL es requerida. Usa: msc wget <URL> o msc wget cookies <URL>")?;

    let folder_name = matches.get_one::<String>("folder");
    let mirror_all = matches.get_flag("all");
    let pattern = matches.get_one::<String>("pattern").map(|s| s.as_str());
    let exclude = matches.get_one::<String>("exclude").map(|s| s.as_str());
    let limit = matches.get_one::<usize>("limit").copied();
    let cookies = matches.get_one::<String>("cookies").map(|s| s.as_str());

    // 2. Validate URL
    validation::validate_web_url(url_str).with_context(|| format!("URL inválida: {}", url_str))?;

    // 3. Ensure wget is installed
    let mut manager = WgetManager::new()?;
    let wget_path = manager.ensure_wget()?;

    // 4. Determine base download directory
    let download_dir = get_download_directory()?;

    // 5. Determine final target directory
    let target_dir = if let Some(name) = folder_name {
        let folder_path = download_dir.join(name);
        ensure_directory_exists(&folder_path)?;
        folder_path
    } else {
        download_dir
    };

    // 6. Execute download
    if mirror_all {
        let mut crawler = Crawler::new(
            url_str, target_dir, wget_path, pattern, exclude, limit, cookies,
        )?;
        crawler.run()?;
    } else {
        execute_download(&wget_path, url_str, &target_dir, false, cookies)?;
        // Post-processing for single page
        println!("{}", "⟳ Procesando HTML para uso offline...".cyan());
        if let Err(e) = process_downloaded_page(url_str, &target_dir) {
            println!(
                "{}",
                format!("⚠️  Error durante el post-procesamiento: {}", e).yellow()
            );
        }
    }

    Ok(())
}

struct Crawler {
    base_url: Url,
    target_dir: PathBuf,
    wget_path: PathBuf,
    visited: HashSet<String>,
    queue: VecDeque<String>,
    pattern_regex: Option<regex::Regex>,
    exclude_regex: Option<regex::Regex>,
    limit: Option<usize>,
    downloaded_count: usize,
    cookie_file: Option<PathBuf>,
}

impl Crawler {
    fn new(
        start_url: &str,
        target_dir: PathBuf,
        wget_path: PathBuf,
        pattern: Option<&str>,
        exclude: Option<&str>,
        limit: Option<usize>,
        cookies: Option<&str>,
    ) -> Result<Self> {
        let base_url = Url::parse(start_url)?;
        let mut queue = VecDeque::new();
        queue.push_back(start_url.to_string());

        // Compile pattern regex if provided
        let pattern_regex = if let Some(p) = pattern {
            match regex::Regex::new(p) {
                Ok(re) => {
                    println!("{} {}", "🔍 Filtro de patrón:".cyan(), p);
                    Some(re)
                }
                Err(e) => {
                    return Err(anyhow!("Patrón regex inválido '{}': {}", p, e));
                }
            }
        } else {
            None
        };

        // Compile exclude regex if provided
        let exclude_regex = if let Some(e) = exclude {
            match regex::Regex::new(e) {
                Ok(re) => {
                    println!("{} {}", "🚫 Excluir patrón:".cyan(), e);
                    Some(re)
                }
                Err(err) => {
                    return Err(anyhow!("Patrón de exclusión inválido '{}': {}", e, err));
                }
            }
        } else {
            None
        };

        // Display limit if specified
        if let Some(l) = limit {
            println!("{} {}", "📊 Límite de páginas:".cyan(), l);
        }

        // Create cookie file if cookies provided
        let cookie_file = if let Some(cookie_str) = cookies {
            // Check if it's a file path
            let path = PathBuf::from(cookie_str);
            if path.exists() && path.is_file() {
                println!("{} {}", "🍪 Cookies:".cyan(), "Archivo cargado".green());
                Some(path)
            } else {
                Some(create_cookie_file(&base_url, cookie_str)?)
            }
        } else {
            None
        };

        Ok(Self {
            base_url,
            target_dir,
            wget_path,
            visited: HashSet::new(),
            queue,
            pattern_regex,
            exclude_regex,
            limit,
            downloaded_count: 0,
            cookie_file,
        })
    }

    fn run(&mut self) -> Result<()> {
        println!();
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
        println!("{}", "  Iniciando Crawler Inteligente".cyan().bold());
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
        println!("{} {}", "🌐 Base URL:".cyan(), self.base_url);
        println!("{} {}", "📁 Destino:".cyan(), self.target_dir.display());
        println!();

        // FASE 1: Crawling - Descargar todo sin modificar hrefs todavía
        while let Some(url) = self.queue.pop_front() {
            // Check limit before processing
            if let Some(limit) = self.limit {
                if self.downloaded_count >= limit {
                    println!();
                    println!(
                        "{}",
                        format!("🛑 Límite alcanzado: {} páginas descargadas", limit)
                            .yellow()
                            .bold()
                    );
                    println!(
                        "{}",
                        format!(
                            "   {} páginas restantes en cola no serán procesadas",
                            self.queue.len()
                        )
                        .dimmed()
                    );
                    break;
                }
            }

            if self.visited.contains(&url) {
                continue;
            }

            // Display progress with limit if applicable
            if let Some(limit) = self.limit {
                println!(
                    "{} {} [{}/{}]",
                    "⬇️  Descargando:".green(),
                    url,
                    self.downloaded_count + 1,
                    limit
                );
            } else {
                println!("{} {}", "⬇️  Descargando:".green(), url);
            }

            // Download the page
            if let Err(e) = self.download_page(&url) {
                log::warn!("Failed to download {}: {}", url, e);
                println!(
                    "{}",
                    format!("⚠️  Falló la descarga de {}: {}", url, e).yellow()
                );
                continue; // Skip processing if download failed
            }

            self.visited.insert(url.clone());
            self.downloaded_count += 1;

            // Extract links only (don't modify hrefs yet)
            println!("   {}", "⟳ Extrayendo enlaces...".dimmed());
            match self.extract_links(&url) {
                Ok(new_links) => {
                    let mut matched_count = 0;
                    let mut filtered_count = 0;

                    for link in new_links {
                        if !self.visited.contains(&link) && !self.queue.contains(&link) {
                            // Apply pattern filter if specified
                            if self.should_crawl_url(&link) {
                                self.queue.push_back(link);
                                matched_count += 1;
                            } else {
                                filtered_count += 1;
                            }
                        }
                    }

                    if matched_count > 0 {
                        println!(
                            "   {}",
                            format!("✓ {} enlaces agregados a la cola", matched_count)
                                .green()
                                .dimmed()
                        );
                    }
                    if filtered_count > 0 && self.pattern_regex.is_some() {
                        println!(
                            "   {}",
                            format!("⊘ {} enlaces filtrados por patrón", filtered_count).dimmed()
                        );
                    }
                }
                Err(e) => {
                    log::warn!("Failed to extract links from {}: {}", url, e);
                    println!(
                        "{}",
                        format!("⚠️  Error extrayendo enlaces de {}: {}", url, e).yellow()
                    );
                }
            }
            println!();
        }

        println!("{}", "✓ Crawling completado".green().bold());
        println!(
            "{}",
            format!("   Total de páginas descargadas: {}", self.downloaded_count).dimmed()
        );
        println!();

        // FASE 2: Post-procesamiento - Ahora que todo está descargado, reemplazar hrefs y recursos
        println!(
            "{}",
            "⟳ Post-procesando archivos para uso offline...".cyan()
        );
        self.post_process_all_files()?;

        println!("{}", "✓ Post-procesamiento finalizado".green().bold());
        Ok(())
    }

    fn download_page(&self, url: &str) -> Result<()> {
        let mut cmd = Command::new(&self.wget_path);

        cmd.arg("--page-requisites") // Download assets
            .arg("--adjust-extension") // Add .html
            .arg("--no-parent") // Don't go up
            .arg("--directory-prefix")
            .arg(&self.target_dir);

        // Add cookie file if provided
        if let Some(ref cookie_file) = self.cookie_file {
            cmd.arg("--load-cookies").arg(cookie_file);
        }

        cmd.arg(url);

        // Note: We do NOT use --convert-links here because we want to rewrite them ourselves
        // and wget's conversion might conflict with our logic or be incomplete for future pages.
        // We do NOT use -nd (no-directories) because we want to preserve structure for the crawler.

        let status = cmd.status().context("Error al ejecutar wget")?;

        if !status.success() {
            // Check for 404/403 but allow continuation if assets failed
            // wget exit code 8 is server error
            if let Some(code) = status.code() {
                if code != 8 {
                    return Err(anyhow!("wget exited with code {}", code));
                }
            }
        }
        Ok(())
    }

    fn extract_links(&self, url: &str) -> Result<Vec<String>> {
        // Determine the local file path for this URL
        let url_parsed = Url::parse(url)?;

        let domain = url_parsed.domain().unwrap_or("unknown");
        let path = url_parsed.path();

        let mut local_path = self.target_dir.join(domain);
        if path == "/" || path.is_empty() {
            local_path.push("index.html");
        } else {
            let rel_path = path.trim_start_matches('/');
            local_path.push(rel_path);

            if path.ends_with('/') {
                local_path.push("index.html");
            } else if !path.ends_with(".html") && !path.ends_with(".htm") {
                local_path.set_extension("html");
            }
        }

        if !local_path.exists() {
            return Err(anyhow!("Local file not found: {}", local_path.display()));
        }

        // Only extract links, don't modify the file yet
        let new_links = extract_links_from_html(&local_path, &self.base_url)?;

        Ok(new_links)
    }

    fn post_process_all_files(&self) -> Result<()> {
        // Iterate over visited URLs and process only those files
        println!(
            "   {}",
            format!(
                "⟳ Procesando {} archivos descargados...",
                self.visited.len()
            )
            .dimmed()
        );

        for url_str in &self.visited {
            if let Ok(url) = Url::parse(url_str) {
                // Calculate local path for this URL
                // Crawler uses directory structure, so use calculate_local_path_for_url
                if let Some(local_path) = calculate_local_path_for_url(&url, &self.target_dir) {
                    if local_path.exists() {
                        // Only process HTML files
                        if local_path
                            .extension()
                            .is_some_and(|ext| ext == "html" || ext == "htm")
                        {
                            println!(
                                "   {}",
                                format!("⟳ Procesando {}", local_path.display()).dimmed()
                            );
                            if let Err(e) = process_html_file_complete(
                                &local_path,
                                &self.target_dir,
                                &self.base_url,
                            ) {
                                println!(
                                    "   {}",
                                    format!("⚠️  Error procesando {}: {}", local_path.display(), e)
                                        .yellow()
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Check if a URL should be crawled based on the pattern filter and exclusion rules
    fn should_crawl_url(&self, url: &str) -> bool {
        // First, check if URL matches exclusion pattern (if provided)
        if let Some(ref exclude) = self.exclude_regex {
            if exclude.is_match(url) {
                return false; // Exclude this URL
            }
        }

        // Parse URL to get the path
        let Ok(parsed_url) = Url::parse(url) else {
            return false;
        };

        // If no inclusion pattern specified, accept all URLs (that weren't excluded)
        let Some(ref pattern) = self.pattern_regex else {
            return true;
        };

        // Get the path (e.g., "/posts/test-article")
        let path = parsed_url.path();

        // Check if path matches the inclusion regex pattern
        pattern.is_match(path)
    }
}

/// Get the download directory (interactive if not configured)
fn get_download_directory() -> Result<PathBuf> {
    let mut config = Config::load()?;

    // Check if web_path is configured
    if let Some(web_path) = config.get_web_path() {
        let path = PathBuf::from(web_path);

        if !path.exists() {
            println!(
                "{}",
                format!(
                    "⚠️  El directorio de webs configurado no existe: {}",
                    web_path
                )
                .yellow()
            );
            println!();

            // Ask for new path interactively
            let new_path = prompt_for_web_path()?;
            save_web_path(&mut config, &new_path)?;
            return Ok(new_path);
        }

        println!("{} {}", "✓ Guardando en:".green(), web_path);
        return Ok(path);
    }

    // No web_path configured, use current directory
    let current_dir = env::current_dir().context("No se pudo obtener el directorio actual")?;

    println!();
    println!("{}", "ℹ️  No hay ruta de webs configurada.".dimmed());
    println!("{}", "   Descargando en el directorio actual.".dimmed());
    println!();
    println!(
        "{}",
        format!(
            "   Configura una ruta permanente con: {}",
            "msc set web <ruta>".cyan()
        )
        .dimmed()
    );
    println!();

    Ok(current_dir)
}

/// Prompt user for web downloads path
fn prompt_for_web_path() -> Result<PathBuf> {
    loop {
        let input: String = Input::new()
            .with_prompt("Ruta del directorio de descargas web")
            .with_initial_text(
                dirs::download_dir()
                    .or_else(|| dirs::home_dir().map(|p| p.join("Downloads")))
                    .and_then(|p| p.to_str().map(String::from))
                    .unwrap_or_default(),
            )
            .interact_text()?;

        let path = PathBuf::from(&input);

        // Check if directory exists
        if !path.exists() {
            println!();
            println!(
                "{}",
                format!("⚠️  El directorio '{}' no existe.", input).yellow()
            );

            // Ask if user wants to create it
            if dialoguer::Confirm::new()
                .with_prompt("¿Deseas crearlo?")
                .default(true)
                .interact()?
            {
                fs::create_dir_all(&path).context("No se pudo crear el directorio")?;
                println!(
                    "{}",
                    format!("✓ Directorio creado: {}", path.display()).green()
                );
                return Ok(path);
            }

            println!("{}", "Por favor, ingresa una ruta válida.".yellow());
            println!();
            continue;
        }

        // Check that it's a directory
        if !path.is_dir() {
            println!();
            println!(
                "{}",
                format!("⚠️  '{}' no es un directorio válido.", input).red()
            );
            println!();
            continue;
        }

        return Ok(path);
    }
}

/// Save web path to configuration
fn save_web_path(config: &mut Config, path: &Path) -> Result<()> {
    let canonical_path = path
        .canonicalize()
        .context("Failed to resolve path")?
        .to_string_lossy()
        .to_string();

    config.set_web_path(canonical_path);
    config.save()?;

    Ok(())
}

/// Ensure directory exists, create if it doesn't
fn ensure_directory_exists(path: &PathBuf) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)
            .with_context(|| format!("No se pudo crear directorio: {}", path.display()))?;
        println!(
            "{}",
            format!("✓ Directorio creado: {}", path.display()).green()
        );
    }
    Ok(())
}

/// Execute the web page download using wget (Single Page Mode)
fn execute_download(
    wget_path: &Path,
    url: &str,
    target_dir: &Path,
    _mirror_all: bool,
    cookies: Option<&str>,
) -> Result<()> {
    print_header(url, target_dir, cookies.is_some());

    let cookie_file_path = resolve_cookie_path(url, cookies)?;

    // --- 3. Construcción del Comando ---
    let mut cmd = Command::new(wget_path);

    cmd.args([
        "--page-requisites",  // Descargar CSS, JS, imágenes
        "--convert-links",    // Hacer links locales
        "--adjust-extension", // Asegurar .html
        "--no-directories",   // Aplanar estructura (-nd)
        "--directory-prefix", // Carpeta destino
    ]);

    // Argumentos con valores dinámicos
    cmd.arg(target_dir);

    // Cookies
    if let Some(path) = &cookie_file_path {
        cmd.arg("--load-cookies").arg(path);
    }

    cmd.arg(url);

    println!("{} {:?}", "Ejecutando:".dimmed(), cmd);
    println!();

    // --- 4. Ejecución ---
    let status = cmd
        .status()
        .context("Error crítico al invocar el binario de wget")?;
    let code = status.code().unwrap_or(-1);

    println!();

    // --- 5. Manejo de Resultados (Wget Exit Codes) ---
    match code {
        0 => {
            println!("{}", "✓ Descarga completada exitosamente".green().bold());
            print_footer(target_dir);
            Ok(())
        }
        8 => {
            println!(
                "{}",
                "⚠️  La descarga completó con advertencias (archivos faltantes/404)."
                    .yellow()
                    .bold()
            );
            println!("{}", "   Continuando con el post-procesamiento...".yellow());
            print_footer(target_dir);
            Ok(())
        }
        _ => {
            // Intento de recuperación: verificar si el archivo principal existe a pesar del error
            handle_download_failure(url, target_dir, code)
        }
    }
}

fn print_header(url: &str, target_dir: &Path, has_cookies: bool) {
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!("{}", "  Descargando Página Web".cyan().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!();
    println!("{} {}", "🌐 URL:".cyan(), url);
    println!("{} {}", "📁 Destino:".cyan(), target_dir.display());
    println!("{} {}", "📋 Modo:".cyan(), "Página única".green());
    if !has_cookies {
        println!("{} {}", "🍪 Cookies:".cyan(), "Ninguna".dimmed());
    }
}

fn print_footer(target_dir: &Path) {
    println!();
    println!(
        "{} {}",
        "📁 Archivos guardados en:".green().bold(),
        target_dir.display()
    );
    println!();
    println!("{}", "Para ver la página, abre el HTML principal.".dimmed());
}

fn handle_download_failure(url: &str, target_dir: &Path, code: i32) -> Result<()> {
    let base_url = Url::parse(url).with_context(|| {
        format!(
            "No se pudo parsear la URL '{}' para verificar archivos locales",
            url
        )
    })?;

    if let Some(main_file) = calculate_flat_local_path(&base_url, target_dir) {
        if main_file.exists() {
            println!(
                "{}",
                format!(
                    "⚠️  Wget falló (código {}) pero el archivo principal existe en: {:?}",
                    code,
                    main_file.file_name().unwrap_or_default()
                )
                .yellow()
            );
            // Retornamos Ok(()) porque consideramos que "el archivo está ahí", así que es un éxito parcial.
            return Ok(());
        }
    }

    Err(anyhow!(
        "La descarga falló. Wget código de salida: {}. El archivo esperado no se encontró.",
        code
    ))
}

/// Extract links from HTML file without modifying it
fn extract_links_from_html(file_path: &PathBuf, base_url: &Url) -> Result<Vec<String>> {
    let content = fs::read_to_string(file_path)?;
    let document = scraper::Html::parse_document(&content);
    let mut extracted_links = Vec::new();

    // 1. Extract links from <a> tags
    let selector = scraper::Selector::parse("a")
        .map_err(|e| anyhow::anyhow!("Failed to create selector: {:?}", e))?;

    for element in document.select(&selector) {
        if let Some(url_str) = element.value().attr("href") {
            // Resolve URL
            if let Ok(resolved_url) = base_url.join(url_str) {
                // Check if it's in scope (same domain)
                if resolved_url.domain() == base_url.domain() {
                    extracted_links.push(resolved_url.to_string());
                }
            }
        }
    }

    // 2. Extract URLs from JSON patterns in scripts (e.g., "nextUrl":"...", "prevUrl":"...")
    // Common patterns in manga/manhwa readers and similar sites
    let json_url_patterns = vec![
        r#""nextUrl"\s*:\s*"([^"]+)""#,
        r#""prevUrl"\s*:\s*"([^"]+)""#,
        r#""next_url"\s*:\s*"([^"]+)""#,
        r#""prev_url"\s*:\s*"([^"]+)""#,
        r#""nextChapter"\s*:\s*"([^"]+)""#,
        r#""prevChapter"\s*:\s*"([^"]+)""#,
    ];

    for pattern in json_url_patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            for cap in re.captures_iter(&content) {
                if let Some(url_match) = cap.get(1) {
                    let url_str = url_match.as_str();

                    // Try to resolve the URL (might be relative or absolute)
                    if let Ok(resolved_url) = base_url.join(url_str) {
                        // Check if it's in scope (same domain)
                        if resolved_url.domain() == base_url.domain() {
                            let url_string = resolved_url.to_string();
                            // Avoid duplicates
                            if !extracted_links.contains(&url_string) {
                                extracted_links.push(url_string);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(extracted_links)
}

/// Post-process directory recursively (for crawler mode after all downloads complete)
fn post_process_directory(current_dir: &PathBuf, root_dir: &PathBuf, base_url: &Url) -> Result<()> {
    let entries = fs::read_dir(current_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Skip assets directory
            if path.file_name().is_some_and(|n| n == "assets") {
                continue;
            }
            post_process_directory(&path, root_dir, base_url)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext == "html" || ext == "htm")
        {
            let msg = format!("⟳ Procesando {}", path.display());
            println!("   {}", msg.dimmed());
            process_html_file_complete(&path, root_dir, base_url)?;
        }
    }
    Ok(())
}

/// Process the downloaded page(s) to ensure all links are local and resources are downloaded (single page mode)
fn process_downloaded_page(original_url: &str, target_dir: &PathBuf) -> Result<()> {
    let base_url = Url::parse(original_url)
        .with_context(|| format!("Invalid URL received: {}", original_url))?;

    // In Single Page Mode, wget uses --no-directories, so the file is in target_dir (flat)
    // We calculate the expected file path and only process that one file
    if let Some(main_file) = calculate_flat_local_path(&base_url, target_dir) {
        if main_file.exists() {
            println!(
                "   {}",
                format!("⟳ Procesando archivo principal: {}", main_file.display()).dimmed()
            );
            process_html_file_complete(&main_file, target_dir, &base_url)?;
        } else {
            // Fallback: if we can't find the specific file, we might warn the user
            // but we explicitly DO NOT want to scan the whole directory to avoid touching other files
            println!(
                "{}",
                format!(
                    "⚠️  No se encontró el archivo principal esperado: {}",
                    main_file.display()
                )
                .yellow()
            );
            println!(
                "{}",
                "   Saltando post-procesamiento para evitar modificar archivos no relacionados."
                    .dimmed()
            );
        }
    } else {
        println!(
            "{}",
            "⚠️  No se pudo determinar el archivo local para la URL.".yellow()
        );
    }

    Ok(())
}

/// Calculate the local file path for a URL assuming a FLAT directory structure
/// (used when wget is run with --no-directories)
fn calculate_flat_local_path(url: &Url, base_dir: &Path) -> Option<PathBuf> {
    let path = url.path();
    let query = url.query(); // Get query parameters
    let mut local_path = base_dir.to_path_buf();

    if path == "/" || path.is_empty() {
        local_path.push("index.html");
    } else {
        // Remove leading slash
        let rel_path = path.trim_start_matches('/');

        // In flat mode, we only care about the filename, not the directory structure
        let mut file_name = if path.ends_with('/') {
            "index.html".to_string()
        } else {
            rel_path
                .split('/')
                .next_back()
                .unwrap_or("index.html")
                .to_string()
        };

        // If there are query parameters, wget converts them to @-notation
        // Example: "page.php?id=123" becomes "page.php@id=123.html"
        // Note: wget does NOT remove the original extension, it just appends @query.html
        if let Some(query_str) = query {
            // Append query with @ separator (keep the original extension)
            file_name = format!("{}@{}", file_name, query_str);
        }

        // Handle extension adjustments similar to wget
        local_path.push(&file_name);

        // wget's behavior with --adjust-extension:
        // - If the file has query params, wget ALWAYS adds .html at the end
        //   Example: "page.php?id=123" -> "page.php@id=123.html"
        //   Example: "page.html?id=123" -> "page.html@id=123.html"
        // - If no query params and no extension -> adds .html
        // - If no query params and non-html extension (like .php) -> adds .html

        if query.is_some() {
            // Always add .html when there are query params
            let name_str = local_path.file_name()?.to_string_lossy().to_string();
            local_path.set_file_name(format!("{}.html", name_str));
        } else {
            // No query params - apply normal wget logic
            let has_extension = file_name.contains('.');
            if !has_extension {
                local_path.set_extension("html");
            } else if !file_name.ends_with(".html") && !file_name.ends_with(".htm") {
                // Check if it's a known resource extension
                let name_buf = PathBuf::from(&file_name);
                let ext = name_buf.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ![
                    "css", "js", "json", "xml", "txt", "pdf", "png", "jpg", "jpeg", "gif", "svg",
                    "ico", "woff", "woff2", "ttf", "eot",
                ]
                .contains(&ext)
                {
                    // Not a known resource, likely HTML content - wget adds .html
                    let name_str = local_path.file_name()?.to_string_lossy().to_string();
                    local_path.set_file_name(format!("{}.html", name_str));
                }
            }
        }

        // Try to find the file - if it doesn't exist with query params, try without
        if !local_path.exists() && query.is_some() {
            // Try without query params
            let simple_name = rel_path.split('/').next_back().unwrap_or("index.html");
            let mut fallback_path = base_dir.to_path_buf();
            fallback_path.push(simple_name);

            let simple_has_extension = simple_name.contains('.');
            if !simple_has_extension {
                fallback_path.set_extension("html");
            } else if !simple_name.ends_with(".html") && !simple_name.ends_with(".htm") {
                let simple_buf = PathBuf::from(simple_name);
                let ext = simple_buf
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                if ![
                    "css", "js", "json", "xml", "txt", "pdf", "png", "jpg", "jpeg", "gif", "svg",
                    "ico", "woff", "woff2", "ttf", "eot",
                ]
                .contains(&ext)
                {
                    let name_str = fallback_path.file_name()?.to_string_lossy().to_string();
                    fallback_path.set_file_name(format!("{}.html", name_str));
                }
            }

            if fallback_path.exists() {
                return Some(fallback_path);
            }
        }
    }

    Some(local_path)
}
