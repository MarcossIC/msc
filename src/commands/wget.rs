use crate::core::{validation, Config, WgetManager};
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use dialoguer::Input;
use std::collections::{HashSet, VecDeque};
use std::env;
use std::fs;
use std::path::PathBuf;
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

        println!(
            "{} {}",
            "⚠️  URL no especificada, usando:".yellow(),
            format!("https://{}", domain)
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

/// Execute the wget command to download web pages
pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    // 1. Extract arguments
    let url_str = matches
        .get_one::<String>("url")
        .context("URL es requerida")?;

    let folder_name = matches.get_one::<String>("folder");
    let mirror_all = matches.get_flag("all");
    let pattern = matches.get_one::<String>("pattern").map(|s| s.as_str());
    let limit = matches.get_one::<usize>("limit").copied();

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
        let mut crawler = Crawler::new(url_str, target_dir, wget_path, pattern, limit)?;
        crawler.run()?;
    } else {
        execute_download(&wget_path, url_str, &target_dir, false)?;
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
    limit: Option<usize>,
    downloaded_count: usize,
}

impl Crawler {
    fn new(
        start_url: &str,
        target_dir: PathBuf,
        wget_path: PathBuf,
        pattern: Option<&str>,
        limit: Option<usize>,
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

        // Display limit if specified
        if let Some(l) = limit {
            println!("{} {}", "📊 Límite de páginas:".cyan(), l);
        }

        Ok(Self {
            base_url,
            target_dir,
            wget_path,
            visited: HashSet::new(),
            queue,
            pattern_regex,
            limit,
            downloaded_count: 0,
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
            .arg(&self.target_dir)
            .arg(url);

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
                            .map_or(false, |ext| ext == "html" || ext == "htm")
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

    /// Check if a URL should be crawled based on the pattern filter
    fn should_crawl_url(&self, url: &str) -> bool {
        // If no pattern specified, accept all URLs
        let Some(ref pattern) = self.pattern_regex else {
            return true;
        };

        // Parse URL to get the path
        let Ok(parsed_url) = Url::parse(url) else {
            return false;
        };

        // Get the path (e.g., "/posts/test-article")
        let path = parsed_url.path();

        // Check if path matches the regex pattern
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
fn save_web_path(config: &mut Config, path: &PathBuf) -> Result<()> {
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
    wget_path: &PathBuf,
    url: &str,
    target_dir: &PathBuf,
    _mirror_all: bool,
) -> Result<()> {
    println!();
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!("{}", "  Descargando Página Web".cyan().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!();
    println!("{} {}", "🌐 URL:".cyan(), url);
    println!("{} {}", "📁 Destino:".cyan(), target_dir.display());
    println!("{} {}", "📋 Modo:".cyan(), "Página única".green());
    println!();

    let mut cmd = Command::new(wget_path);

    // Single page mode: only download the specific page and its resources
    // We use -nd to flatten structure for single page convenience (as per original logic)
    cmd.arg("--page-requisites") // Download all page assets (CSS, images, JS)
        .arg("--convert-links") // Convert links for offline viewing
        .arg("--adjust-extension") // Add .html extension to files
        .arg("--no-directories"); // Don't create directory structure

    // Common arguments
    cmd.arg("--directory-prefix") // Set download directory
        .arg(target_dir)
        .arg(url);

    println!("{} {:?}", "Ejecutando:".dimmed(), cmd);
    println!();

    // Execute command
    let status = cmd.status().context("Error al ejecutar wget")?;

    println!();

    let success = status.success();
    let code = status.code().unwrap_or(-1);

    if success || code == 8 {
        if success {
            println!("{}", "✓ Descarga completada exitosamente".green().bold());
        } else {
            println!(
                "{}",
                "⚠️  La descarga completó con algunos errores (archivos faltantes/404)."
                    .yellow()
                    .bold()
            );
            println!("{}", "   Continuando con el post-procesamiento...".yellow());
        }

        println!();
        println!(
            "{} {}",
            "📁 Archivos guardados en:".green().bold(),
            target_dir.display()
        );
        println!();
        println!(
            "{}",
            "Para ver la página offline, abre el archivo HTML principal en tu navegador.".dimmed()
        );
    } else {
        return Err(anyhow!(
            "La descarga falló con código de salida: {}",
            status
        ));
    }

    Ok(())
}

/// Extract links from HTML file without modifying it
fn extract_links_from_html(file_path: &PathBuf, base_url: &Url) -> Result<Vec<String>> {
    let content = fs::read_to_string(file_path)?;
    let document = scraper::Html::parse_document(&content);
    let mut extracted_links = Vec::new();

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
            if path.file_name().map_or(false, |n| n == "assets") {
                continue;
            }
            post_process_directory(&path, root_dir, base_url)?;
        } else if path
            .extension()
            .map_or(false, |ext| ext == "html" || ext == "htm")
        {
            println!("   {}", format!("⟳ Procesando {}", path.display()).dimmed());
            process_html_file_complete(&path, root_dir, base_url)?;
        }
    }
    Ok(())
}

/// Process the downloaded page(s) to ensure all links are local and resources are downloaded (single page mode)
fn process_downloaded_page(original_url: &str, target_dir: &PathBuf) -> Result<()> {
    let base_url =
        Url::parse(original_url).unwrap_or_else(|_| Url::parse("http://example.com").unwrap());

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

/// Calculate possible local file paths where the URL might be saved
/// Returns multiple possibilities to handle both flat and nested directory structures
fn calculate_possible_local_paths(url: &Url, base_dir: &PathBuf) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let domain = match url.domain() {
        Some(d) => d,
        None => return paths,
    };

    let url_path = url.path();

    // Build the relative path from URL
    let mut rel_path_parts = Vec::new();

    if url_path == "/" || url_path.is_empty() {
        rel_path_parts.push("index.html".to_string());
    } else {
        let rel_path = url_path.trim_start_matches('/');

        if url_path.ends_with('/') {
            rel_path_parts.push(rel_path.to_string());
            rel_path_parts.push("index.html".to_string());
        } else {
            rel_path_parts.push(rel_path.to_string());

            // Add .html extension if needed
            let has_extension = rel_path.contains('.')
                && rel_path
                    .split('/')
                    .last()
                    .map(|f| f.contains('.'))
                    .unwrap_or(false);

            if !has_extension {
                // Modify last part to add .html
                if let Some(last) = rel_path_parts.last_mut() {
                    *last = format!("{}.html", last);
                }
            } else if !url_path.ends_with(".html") && !url_path.ends_with(".htm") {
                let ext = rel_path.split('.').last().unwrap_or("");
                if ![
                    "html", "htm", "css", "js", "json", "xml", "txt", "pdf", "png", "jpg", "jpeg",
                    "gif", "svg", "ico", "woff", "woff2", "ttf", "eot",
                ]
                .contains(&ext)
                {
                    if let Some(last) = rel_path_parts.last_mut() {
                        *last = format!("{}.html", last);
                    }
                }
            }
        }
    }

    // Build final path string
    let final_rel_path = rel_path_parts.join("");

    // Check if base_dir ends with the domain name
    // This works for both relative (./manhwa-espanol.com) and absolute paths
    // (C:\Users\...\manhwa-espanol.com)
    let base_dir_name = base_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");

    if base_dir_name == domain {
        // base_dir already includes domain, so try both options:
        // Option 1: base_dir/path (without domain duplication - CORRECT)
        let path_without_domain = base_dir.join(&final_rel_path);
        paths.push(path_without_domain);

        // Option 2: base_dir/domain/path (with domain duplication - legacy/alternative)
        let path_with_domain = base_dir.join(domain).join(&final_rel_path);
        paths.push(path_with_domain);
    } else {
        // base_dir doesn't include domain, so only try with domain
        let path_with_domain = base_dir.join(domain).join(&final_rel_path);
        paths.push(path_with_domain);
    }

    paths
}

/// Calculate the local file path for a URL assuming a FLAT directory structure
/// (used when wget is run with --no-directories)
fn calculate_flat_local_path(url: &Url, base_dir: &PathBuf) -> Option<PathBuf> {
    let path = url.path();
    let mut local_path = base_dir.clone();

    if path == "/" || path.is_empty() {
        local_path.push("index.html");
    } else {
        // Remove leading slash
        let rel_path = path.trim_start_matches('/');

        // In flat mode, we only care about the filename, not the directory structure
        let file_name = if path.ends_with('/') {
            "index.html"
        } else {
            rel_path.split('/').last().unwrap_or("index.html")
        };

        // Handle extension adjustments similar to wget
        let name_buf = PathBuf::from(file_name);

        // If it looks like a file but has no extension, wget adds .html
        // Or if it has an extension that is not typical for files, wget might add .html
        // But simpler logic: check if the file exists as is, or with .html

        local_path.push(file_name);

        // If the file doesn't exist as is, maybe wget added .html?
        // But here we are calculating where it SHOULD be.
        // wget logic:
        // if content-type is html and extension is not html/htm -> add .html
        // We don't know content-type here, but we can guess.

        let has_extension = file_name.contains('.');
        if !has_extension {
            local_path.set_extension("html");
        } else if !file_name.ends_with(".html") && !file_name.ends_with(".htm") {
            // Check if it's a known non-html extension
            let ext = name_buf.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ![
                "css", "js", "json", "xml", "txt", "pdf", "png", "jpg", "jpeg", "gif", "svg",
                "ico", "woff", "woff2", "ttf", "eot",
            ]
            .contains(&ext)
            {
                // It might have .html appended
                // But we can't be sure without checking if the file exists.
                // Since this function returns ONE path, we have to guess.
                // Let's return the one with .html if the original doesn't look like a resource.
                let name_str = local_path.file_name()?.to_string_lossy().to_string();
                local_path.set_file_name(format!("{}.html", name_str));
            }
        }
    }

    Some(local_path)
}

/// Calculate the local file path where wget would save a given URL
/// This mirrors wget's behavior with --adjust-extension and directory structure
fn calculate_local_path_for_url(url: &Url, base_dir: &PathBuf) -> Option<PathBuf> {
    let domain = url.domain()?;
    let path = url.path();

    let mut local_path = base_dir.join(domain);

    if path == "/" || path.is_empty() {
        local_path.push("index.html");
    } else {
        // Remove leading slash and decode percent encoding
        let rel_path = path.trim_start_matches('/');

        if path.ends_with('/') {
            // Directory path - wget adds index.html
            local_path.push(rel_path);
            local_path.push("index.html");
        } else {
            // File path
            local_path.push(rel_path);

            // wget adds .html extension if the path doesn't have a typical file extension
            // and content-type is text/html
            let has_extension = rel_path.contains('.')
                && rel_path
                    .split('/')
                    .last()
                    .map(|f| f.contains('.'))
                    .unwrap_or(false);

            if !has_extension {
                // No extension - wget will add .html
                local_path.set_extension("html");
            } else {
                // Has extension, but if it's not .html or .htm, wget might still add .html
                // For simplicity, we assume HTML pages have .html/.htm or no extension
                if !path.ends_with(".html") && !path.ends_with(".htm") {
                    // Check if it looks like a HTML page (no other file extension)
                    let ext = local_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    if ext != "html"
                        && ext != "htm"
                        && ![
                            "css", "js", "json", "xml", "txt", "pdf", "png", "jpg", "jpeg", "gif",
                            "svg", "ico", "woff", "woff2", "ttf", "eot",
                        ]
                        .contains(&ext)
                    {
                        // Likely an HTML page with unusual extension - wget adds .html
                        let new_name =
                            format!("{}.html", local_path.file_name()?.to_string_lossy());
                        local_path.set_file_name(new_name);
                    }
                }
            }
        }
    }

    Some(local_path)
}

/// Process HTML file completely: download resources, rewrite resource URLs, and rewrite hrefs to local files
fn process_html_file_complete(
    file_path: &PathBuf,
    base_dir: &PathBuf,
    base_url: &Url,
) -> Result<()> {
    let content = fs::read_to_string(file_path)?;
    let document = scraper::Html::parse_document(&content);

    let mut new_content = content.clone();
    let mut replacements = Vec::new();

    let selector = scraper::Selector::parse("img, script, link, a")
        .map_err(|e| anyhow::anyhow!("Failed to create selector: {:?}", e))?;

    // Assets directory is now NEXT TO the HTML file (same directory)
    // So the relative path is simply "assets/"
    let parent = file_path.parent().unwrap_or(base_dir);
    let assets_rel_path = "assets/";

    for element in document.select(&selector) {
        let tag_name = element.value().name();

        // Handle lazy loading for images
        let attr_name = match tag_name {
            "a" | "link" => "href",
            "img" | "script" => "src",
            _ => continue,
        };

        let mut url_val = element.value().attr(attr_name);

        // Lazy loading hydration logic for images
        if tag_name == "img" {
            let lazy_attrs = ["data-src", "data-original", "data-lazy-src", "data-url"];
            for attr in lazy_attrs {
                if let Some(val) = element.value().attr(attr) {
                    if !val.is_empty() {
                        url_val = Some(val);
                        break;
                    }
                }
            }
        }

        if let Some(url_str) = url_val {
            // Check ignore rules
            if url_str.contains("fonts.googleapis.com")
                || url_str.contains("fonts.gstatic.com")
                || url_str.ends_with(".php")
                || url_str.contains("xmlrpc.php")
            {
                continue;
            }

            // Handle <a> links - Replace with local paths
            if tag_name == "a" {
                // Resolve URL
                if let Ok(resolved_url) = base_url.join(url_str) {
                    // Check if it's in scope (same domain)
                    if resolved_url.domain() == base_url.domain() {
                        // Calculate the local path where this file should be
                        if let Some(local_link_path) =
                            calculate_local_path_for_url(&resolved_url, base_dir)
                        {
                            // IMPORTANT: Only replace if the file actually exists
                            if local_link_path.exists() {
                                // Calculate relative path from current file to the linked file
                                if let Some(relative_link) =
                                    pathdiff::diff_paths(&local_link_path, parent)
                                {
                                    let relative_link_str =
                                        relative_link.to_string_lossy().replace('\\', "/"); // Normalize path separators for HTML

                                    // Add replacement for this link
                                    replacements.push((
                                        url_str.to_string(),
                                        relative_link_str,
                                        false,
                                    ));
                                }
                            }
                        }
                    }
                }
                continue;
            }

            // Resource Downloading (Images, JS, CSS)
            if url_str.starts_with("http://")
                || url_str.starts_with("https://")
                || url_str.starts_with("//")
            {
                let full_url = if url_str.starts_with("//") {
                    format!("https:{}", url_str)
                } else {
                    url_str.to_string()
                };

                // Determine local filename
                let file_name = match full_url.split('/').last() {
                    Some(name) if !name.is_empty() => {
                        // Remove query params
                        name.split('?').next().unwrap_or("resource")
                    }
                    _ => "resource",
                };

                // Add extension if missing
                let file_name = if !file_name.contains('.') {
                    match tag_name {
                        "img" => format!("{}.jpg", file_name),
                        "script" => format!("{}.js", file_name),
                        "link" => format!("{}.css", file_name),
                        _ => file_name.to_string(),
                    }
                } else {
                    file_name.to_string()
                };

                // Try to find or download the resource
                // Priority: 1) local assets/ folder, 2) global assets/ folder
                let local_assets_dir = parent.join("assets");
                let global_assets_dir = base_dir.join("assets");

                let local_path = local_assets_dir.join(&file_name);
                let global_path = global_assets_dir.join(&file_name);

                let (final_path, relative_path) = if local_path.exists() {
                    // Already exists locally
                    (local_path, format!("{}{}", assets_rel_path, file_name))
                } else if global_path.exists() {
                    // Exists in global, calculate relative path from current file to global
                    let parent_to_base = pathdiff::diff_paths(base_dir, parent).unwrap_or_default();
                    let mut rel_to_global = String::new();
                    for _ in 0..parent_to_base.components().count() {
                        rel_to_global.push_str("../");
                    }
                    rel_to_global.push_str("assets/");
                    rel_to_global.push_str(&file_name);
                    (global_path, rel_to_global)
                } else {
                    // Need to download - prefer local
                    fs::create_dir_all(&local_assets_dir)?;
                    (local_path, format!("{}{}", assets_rel_path, file_name))
                };

                // Download if needed
                if !final_path.exists() {
                    match download_resource(&full_url, &final_path) {
                        Ok(_) => {}
                        Err(e) => {
                            log::warn!("Failed to download {}: {}", full_url, e);
                            continue;
                        }
                    }
                }

                // Record replacement
                if tag_name == "img" {
                    // Check if the ORIGINAL src attribute was a placeholder
                    if let Some(original_src) = element.value().attr("src") {
                        if is_placeholder_image(original_src) {
                            replacements.push((
                                original_src.to_string(),
                                relative_path.clone(),
                                true,
                            ));
                        }
                    }
                    replacements.push((url_str.to_string(), relative_path, true));
                } else {
                    replacements.push((url_str.to_string(), relative_path, false));
                }
            }
        }
    }

    // Apply replacements
    for (target, replacement, is_image) in replacements {
        if is_image {
            // Try to find the target. It might be HTML encoded in the file (e.g. & -> &amp;)
            let target_encoded = target.replace("&", "&amp;");

            let targets_to_try = if target != target_encoded {
                vec![target.clone(), target_encoded]
            } else {
                vec![target.clone()]
            };

            let mut replaced_any = false;

            for curr_target in targets_to_try {
                let mut search_start = 0;
                while let Some(idx) = new_content[search_start..].find(&curr_target) {
                    let absolute_idx = search_start + idx;

                    let tag_start = new_content[..absolute_idx].rfind("<img");

                    if let Some(start) = tag_start {
                        let tag_end_offset = new_content[absolute_idx..].find('>');

                        if let Some(end_offset) = tag_end_offset {
                            let end = absolute_idx + end_offset + 1;
                            let tag_content = &new_content[start..end];

                            if tag_content.contains(&curr_target) {
                                let mut new_tag = tag_content.to_string();
                                new_tag = new_tag.replace(&curr_target, &replacement);

                                let src_pattern = regex::Regex::new(r#"src\s*=\s*["'][^"']*["']"#)
                                    .context("Failed to create regex pattern")?;
                                new_tag = src_pattern
                                    .replace(&new_tag, format!("src=\"{}\"", replacement).as_str())
                                    .to_string();

                                new_content.replace_range(start..end, &new_tag);
                                search_start = start + new_tag.len();
                                replaced_any = true;
                                continue;
                            }
                        }
                    }
                    search_start = absolute_idx + curr_target.len();
                }
            }

            if !replaced_any {
                new_content = new_content.replace(&target, &replacement);
            }
        } else {
            new_content = new_content.replace(&target, &replacement);
        }
    }

    // --- Script-based Image Extraction (ts_reader) ---
    let script_regex =
        regex::Regex::new(r#"ts_reader\.run\((.*)\);"#).context("Failed to create script regex")?;
    let mut script_replacements = Vec::new();

    for cap in script_regex.captures_iter(&new_content) {
        if let Some(json_match) = cap.get(1) {
            let json_str = json_match.as_str();
            if let Ok(mut json_data) = serde_json::from_str::<serde_json::Value>(json_str) {
                let mut modified = false;

                if let Some(obj) = json_data.as_object_mut() {
                    obj.insert("lazyload".to_string(), serde_json::Value::Bool(false));
                    modified = true;

                    // Replace nextUrl with local path if it exists
                    if let Some(next_url_val) = obj.get("nextUrl") {
                        if let Some(next_url_str) = next_url_val.as_str() {
                            // Parse and resolve the nextUrl
                            let resolved_url = if next_url_str.starts_with("http://")
                                || next_url_str.starts_with("https://")
                            {
                                Url::parse(next_url_str).ok()
                            } else {
                                base_url.join(next_url_str).ok()
                            };

                            if let Some(resolved_url) = resolved_url {
                                // Check if it's in the same domain
                                if resolved_url.domain() == base_url.domain() {
                                    // Calculate possible local paths for this URL
                                    let possible_paths =
                                        calculate_possible_local_paths(&resolved_url, base_dir);

                                    // Try each possible path until we find one that exists
                                    let mut found_path: Option<PathBuf> = None;
                                    for path in possible_paths.iter() {
                                        if path.exists() {
                                            found_path = Some(path.clone());
                                            break;
                                        }
                                    }

                                    if let Some(local_next_path) = found_path {
                                        // Calculate relative path from current file to next file
                                        let parent_file = file_path.parent().unwrap_or(base_dir);
                                        if let Some(relative_next) =
                                            pathdiff::diff_paths(&local_next_path, parent_file)
                                        {
                                            let relative_next_str =
                                                relative_next.to_string_lossy().replace('\\', "/");

                                            obj.insert(
                                                "nextUrl".to_string(),
                                                serde_json::Value::String(relative_next_str),
                                            );
                                            modified = true;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Similarly handle prevUrl if it exists
                    if let Some(prev_url_val) = obj.get("prevUrl") {
                        if let Some(prev_url_str) = prev_url_val.as_str() {
                            let resolved_url = if prev_url_str.starts_with("http://")
                                || prev_url_str.starts_with("https://")
                            {
                                Url::parse(prev_url_str).ok()
                            } else {
                                base_url.join(prev_url_str).ok()
                            };

                            if let Some(resolved_url) = resolved_url {
                                if resolved_url.domain() == base_url.domain() {
                                    let possible_paths =
                                        calculate_possible_local_paths(&resolved_url, base_dir);

                                    let mut found_path: Option<PathBuf> = None;
                                    for path in possible_paths.iter() {
                                        if path.exists() {
                                            found_path = Some(path.clone());
                                            break;
                                        }
                                    }

                                    if let Some(local_prev_path) = found_path {
                                        let parent_file = file_path.parent().unwrap_or(base_dir);
                                        if let Some(relative_prev) =
                                            pathdiff::diff_paths(&local_prev_path, parent_file)
                                        {
                                            let relative_prev_str =
                                                relative_prev.to_string_lossy().replace('\\', "/");

                                            obj.insert(
                                                "prevUrl".to_string(),
                                                serde_json::Value::String(relative_prev_str),
                                            );
                                            modified = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(sources) = json_data.get_mut("sources").and_then(|s| s.as_array_mut()) {
                    for source in sources {
                        if let Some(images) =
                            source.get_mut("images").and_then(|i| i.as_array_mut())
                        {
                            for image_val in images {
                                if let Some(img_url) = image_val.as_str() {
                                    // Skip if already a local relative path
                                    if is_local_path(img_url) {
                                        continue; // Already processed, skip
                                    }

                                    let full_url = if img_url.starts_with("//") {
                                        format!("https:{}", img_url)
                                    } else if img_url.starts_with("http://")
                                        || img_url.starts_with("https://")
                                    {
                                        img_url.to_string()
                                    } else {
                                        // Relative URL that needs to be resolved against base_url
                                        match base_url.join(img_url) {
                                            Ok(resolved) => resolved.to_string(),
                                            Err(_) => {
                                                // Can't resolve, skip it
                                                continue;
                                            }
                                        }
                                    };

                                    let file_name = match full_url.split('/').last() {
                                        Some(name) if !name.is_empty() => {
                                            name.split('?').next().unwrap_or("image.jpg")
                                        }
                                        _ => "image.jpg",
                                    };

                                    // Try to find or download the resource
                                    // Priority: 1) local assets/ folder, 2) global assets/ folder
                                    let local_assets_dir = parent.join("assets");
                                    let global_assets_dir = base_dir.join("assets");

                                    let local_path = local_assets_dir.join(&file_name);
                                    let global_path = global_assets_dir.join(&file_name);

                                    let (final_path, replacement_path) = if local_path.exists() {
                                        // Already exists locally
                                        (local_path, format!("{}{}", assets_rel_path, file_name))
                                    } else if global_path.exists() {
                                        // Exists in global, calculate relative path
                                        let parent_to_base = pathdiff::diff_paths(base_dir, parent)
                                            .unwrap_or_default();
                                        let mut rel_to_global = String::new();
                                        for _ in 0..parent_to_base.components().count() {
                                            rel_to_global.push_str("../");
                                        }
                                        rel_to_global.push_str("assets/");
                                        rel_to_global.push_str(&file_name);
                                        (global_path, rel_to_global)
                                    } else {
                                        // Need to download - prefer local
                                        if let Ok(_) = fs::create_dir_all(&local_assets_dir) {
                                            (
                                                local_path,
                                                format!("{}{}", assets_rel_path, file_name),
                                            )
                                        } else {
                                            continue;
                                        }
                                    };

                                    // Download if needed
                                    if !final_path.exists() {
                                        match download_resource(&full_url, &final_path) {
                                            Ok(_) => {
                                                *image_val =
                                                    serde_json::Value::String(replacement_path);
                                                modified = true;
                                            }
                                            Err(e) => {
                                                log::warn!(
                                                    "Failed to download script image {}: {}",
                                                    full_url,
                                                    e
                                                );
                                            }
                                        }
                                    } else {
                                        // File already exists, just update the reference
                                        *image_val = serde_json::Value::String(replacement_path);
                                        modified = true;
                                    }
                                }
                            }
                        }
                    }
                }

                if modified {
                    if let Ok(new_json_str) = serde_json::to_string(&json_data) {
                        script_replacements.push((json_str.to_string(), new_json_str));
                    }
                }
            }
        }
    }

    for (old_json, new_json) in script_replacements {
        new_content = new_content.replace(&old_json, &new_json);
    }

    fs::write(file_path, new_content)?;

    Ok(())
}

/// Check if a path is already a local relative path (not a remote URL)
fn is_local_path(path: &str) -> bool {
    // Check if it's a local relative path
    path.starts_with("./")
        || path.starts_with("../")
        || path.starts_with("assets/")
        || path.starts_with("../assets/")
        || path.starts_with("../../assets/")
        || (!path.starts_with("http://")
            && !path.starts_with("https://")
            && !path.starts_with("//")
            && !path.starts_with("/") // Absolute paths from root are not local relative
            && path.contains("assets")) // Contains 'assets' but no protocol
}

fn is_placeholder_image(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    let valid_extensions = [".jpg", ".png", ".gif", ".svg", ".webp"];
    let valid_names = [
        "loading",
        "readerarea",
        "reader",
        "launching",
        "placeholder",
        "skeleton",
        "spinner",
        "indicator",
        "loader",
    ];

    let has_valid_ext = valid_extensions.iter().any(|ext| url_lower.ends_with(ext));
    if !has_valid_ext {
        return false;
    }

    valid_names.iter().any(|name| url_lower.contains(name))
}

fn download_resource(url: &str, path: &PathBuf) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    let response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        return Err(anyhow!("Status: {}", response.status()));
    }

    let bytes = response.bytes()?;
    fs::write(path, bytes)?;
    Ok(())
}
