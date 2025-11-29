use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{PathBuf};
use url::Url;

use super::wget_utils::{
    calculate_local_path_for_url, calculate_possible_local_paths,
    download_resource, extract_filename_from_url, is_local_path, is_placeholder_image
};
use crate::core::validation::{load_default_blacklist, validate_url_not_blacklisted};

/// Process HTML file completely: download resources, rewrite resource URLs, and rewrite hrefs to local files
pub fn process_html_file_complete(
    file_path: &PathBuf,
    base_dir: &PathBuf,
    base_url: &Url,
) -> Result<()> {
    let content = fs::read_to_string(file_path)?;
    let document = scraper::Html::parse_document(&content);

    let mut new_content = content.clone();
    let mut replacements = Vec::new();

    // Load blacklist for URL filtering
    let blacklist = match load_default_blacklist() {
        Ok(bl) => {
            if !bl.is_empty() {
                println!("   {} {} dominios bloqueados",
                    "🛡️  Blacklist cargada:".dimmed(),
                    bl.len().to_string().cyan()
                );
            }
            bl
        }
        Err(e) => {
            println!("   {} {}",
                "⚠️  No se pudo cargar blacklist:".yellow().dimmed(),
                e.to_string().dimmed()
            );
            // Continue without blacklist
            crate::core::Blacklist::new()
        }
    };

    let selector = scraper::Selector::parse("img, script, link, a, iframe")
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
            "img" | "script" | "iframe" => "src",
            _ => continue,
        };

        let mut url_val = element.value().attr(attr_name);
        let mut srcset_urls: Vec<String> = Vec::new();

        // Handle srcset attribute for images (multiple URLs with different sizes)
        if tag_name == "img" {
            if let Some(srcset_val) = element.value().attr("srcset") {
                if !srcset_val.is_empty() {
                    // Parse srcset: "url1 size1, url2 size2, ..."
                    // Split by comma and extract each URL
                    for srcset_entry in srcset_val.split(',') {
                        let trimmed = srcset_entry.trim();
                        // Each entry is "URL SIZE" (e.g., "https://example.com/img.webp 300w")
                        // Extract just the URL part (before the space)
                        if let Some(url_part) = trimmed.split_whitespace().next() {
                            if !url_part.is_empty() &&
                               (url_part.starts_with("http://") ||
                                url_part.starts_with("https://") ||
                                url_part.starts_with("//")) {
                                srcset_urls.push(url_part.to_string());
                            }
                        }
                    }
                }
            }

            // Lazy loading hydration logic for images
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
                || url_str.contains("googletagmanager.com")
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

                // Check blacklist before downloading
                if let Err(e) = validate_url_not_blacklisted(&full_url, &blacklist) {
                    println!("   {} {} - {}",
                        "🚫 URL bloqueada:".red().dimmed(),
                        full_url.dimmed(),
                        e.to_string().yellow()
                    );
                    continue; // Skip this resource
                }

                // Log external resource detection for images
                if tag_name == "img" {
                    println!("   {} {} ({})",
                        "🔍 Detectada imagen externa:".blue().dimmed(),
                        full_url.dimmed(),
                        tag_name.yellow()
                    );
                }

                // Extract safe filename from URL
                let file_name = extract_filename_from_url(&full_url);

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
                        Ok(_) => {
                            println!("   {} {} -> {}",
                                "✓ Descargado:".green().dimmed(),
                                full_url.dimmed(),
                                file_name.cyan()
                            );
                        }
                        Err(e) => {
                            println!("   {} {} -> Error: {}",
                                "✗ Error descargando:".red().dimmed(),
                                full_url.dimmed(),
                                e.to_string().yellow()
                            );
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

        // Process srcset URLs (for images with multiple size variants)
        for srcset_url in srcset_urls {
            // Check ignore rules for srcset URLs
            if srcset_url.contains("fonts.googleapis.com")
                || srcset_url.contains("fonts.gstatic.com")
                || srcset_url.contains("googletagmanager.com")
                || srcset_url.ends_with(".php")
                || srcset_url.contains("xmlrpc.php")
            {
                continue;
            }

            // Process external resource from srcset
            if srcset_url.starts_with("http://")
                || srcset_url.starts_with("https://")
                || srcset_url.starts_with("//")
            {
                let full_url = if srcset_url.starts_with("//") {
                    format!("https:{}", srcset_url)
                } else {
                    srcset_url.clone()
                };

                // Check blacklist before downloading srcset images
                if let Err(e) = validate_url_not_blacklisted(&full_url, &blacklist) {
                    println!("   {} {} - {}",
                        "🚫 URL bloqueada (srcset):".red().dimmed(),
                        full_url.dimmed(),
                        e.to_string().yellow()
                    );
                    continue; // Skip this srcset resource
                }

                println!("   {} {} (srcset)",
                    "🔍 Detectada imagen en srcset:".blue().dimmed(),
                    full_url.dimmed()
                );

                // Extract safe filename from URL
                let file_name = extract_filename_from_url(&full_url);

                // Try to find or download the resource
                let local_assets_dir = parent.join("assets");
                let global_assets_dir = base_dir.join("assets");

                let local_path = local_assets_dir.join(&file_name);
                let global_path = global_assets_dir.join(&file_name);

                let (final_path, relative_path) = if local_path.exists() {
                    (local_path, format!("{}{}", assets_rel_path, file_name))
                } else if global_path.exists() {
                    let parent_to_base = pathdiff::diff_paths(base_dir, parent).unwrap_or_default();
                    let mut rel_to_global = String::new();
                    for _ in 0..parent_to_base.components().count() {
                        rel_to_global.push_str("../");
                    }
                    rel_to_global.push_str("assets/");
                    rel_to_global.push_str(&file_name);
                    (global_path, rel_to_global)
                } else {
                    fs::create_dir_all(&local_assets_dir)?;
                    (local_path, format!("{}{}", assets_rel_path, file_name))
                };

                // Download if needed
                if !final_path.exists() {
                    match download_resource(&full_url, &final_path) {
                        Ok(_) => {
                            println!("   {} {} -> {}",
                                "✓ Descargado (srcset):".green().dimmed(),
                                full_url.dimmed(),
                                file_name.cyan()
                            );
                        }
                        Err(e) => {
                            println!("   {} {} -> Error: {}",
                                "✗ Error descargando (srcset):".red().dimmed(),
                                full_url.dimmed(),
                                e.to_string().yellow()
                            );
                            log::warn!("Failed to download srcset image {}: {}", full_url, e);
                            continue;
                        }
                    }
                }

                // Record replacement for srcset URL
                replacements.push((srcset_url.clone(), relative_path, true));
            }
        }
    }

    // Apply replacements
    let src_pattern = regex::Regex::new(r#"src\s*=\s*["'][^"']*["']"#)
        .context("Failed to create regex pattern")?;

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

                                    let file_name = extract_filename_from_url(&full_url);

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
                                        if fs::create_dir_all(&local_assets_dir).is_ok() {
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

    // --- Script Sanitization (Remove Redirects) ---
    // Remove scripts containing "redirectUrl =" or "createAccountUrl ="
    let script_block_regex = regex::Regex::new(r#"(?s)<script[^>]*>(.*?)</script>"#)
        .context("Failed to create script block regex")?;
    
    let mut script_removals = Vec::new();

for cap in script_block_regex.captures_iter(&new_content) {
    if let (Some(full_match), Some(content_match)) = (cap.get(0), cap.get(1)) {
        let script_content = content_match.as_str();
        let full_script = full_match.as_str();

        // Verificar si es un script externo que carga cookie_banner.js
        let is_cookie_banner_src = full_script.contains("src=") &&
                                   (full_script.contains("cookie_banner.js") ||
                                    full_script.contains("cookie_banner.min.js"));

        // Verificar si es un script de redirección o compra
        let is_redirect_script = script_content.contains("redirectUrl =") ||
                                script_content.contains("createAccountUrl =");
                                // script_content.contains("createAccountUrl =") ||
                                // script_content.contains("var PURCHASE_PHX =");

        // Verificar si es el script del banner de cookies (inline)
        let is_cookie_banner_script = script_content.contains("typeof CookieHelper != 'undefined'") ||
                                     script_content.contains("typeof CookieHelper !== 'undefined'") ||
                                     script_content.contains("var essentialCookiesListAll =") ||
                                     script_content.contains("var customizeCookiesTemplate =");

        // Verificar si es un script de publicidad
        let is_ads_script = false;

        // Verificar si es un script de Google Tag Manager o analytics
        let is_analytics_script = full_script.contains("https://www.googletagmanager.com/gtm.js") ||
                                 full_script.contains("googletagmanager.com/gtm.js");

        // Verificar si es un script de CAPTCHA
        let is_captcha_script = script_content.contains("captchaType") ||
                               script_content.contains("captchaToken") ||
                               full_script.contains("www.google.com/recaptcha");

        // Verificar si es un script con funciones de cookies/mensajes de usuario
        let is_cookie_message_script = full_script.contains(r#"<a onclick="hideUserMessage();"#) ||
                                       script_content.contains("setCookieAdvanced(");

        // Verificar si es un script relacionado con Google/bots/whitelist
        let is_bot_whitelist_script = script_content.contains("'google'") ||
                                      script_content.contains("'googlebot'") ||
                                      script_content.contains("isInWhitelist");

        // Verificar si es un script de geo-localización
        let is_geo_localization_script = script_content.contains("geo-localization") ||
                                         script_content.contains("geolocalization") ||
                                         script_content.contains("geo_localization");

        if is_redirect_script || is_cookie_banner_script || is_cookie_banner_src || is_ads_script || is_analytics_script || is_captcha_script || is_cookie_message_script || is_bot_whitelist_script || is_geo_localization_script {
            script_removals.push(full_script.to_string());

            // Mostrar mensaje específico según el tipo de script
            if is_cookie_banner_script || is_cookie_banner_src {
                println!("   {} {}", "🍪 Detectado script de banner de cookies".yellow().dimmed(),
                        format!("({} bytes)", full_script.len()).dimmed());
            } else if is_ads_script {
                println!("   {} {}", "📢 Detectado script de publicidad".yellow().dimmed(),
                        format!("({} bytes)", full_script.len()).dimmed());
            } else if is_analytics_script {
                println!("   {} {}", "📊 Detectado script de analytics/tracking".yellow().dimmed(),
                        format!("({} bytes)", full_script.len()).dimmed());
            } else if is_captcha_script {
                println!("   {} {}", "🤖 Detectado script de CAPTCHA/reCAPTCHA".yellow().dimmed(),
                        format!("({} bytes)", full_script.len()).dimmed());
            } else if is_cookie_message_script {
                println!("   {} {}", "💬 Detectado script de mensajes/cookies de usuario".yellow().dimmed(),
                        format!("({} bytes)", full_script.len()).dimmed());
            } else if is_bot_whitelist_script {
                println!("   {} {}", "🔍 Detectado script de bot/whitelist detection".yellow().dimmed(),
                        format!("({} bytes)", full_script.len()).dimmed());
            } else if is_geo_localization_script {
                println!("   {} {}", "🌍 Detectado script de geo-localización".yellow().dimmed(),
                        format!("({} bytes)", full_script.len()).dimmed());
            }
        }
    }
}

    if !script_removals.is_empty() {
        println!("   {}", format!("🛡️  Eliminando {} scripts de redirección...", script_removals.len()).yellow().dimmed());
        for script in script_removals {
            new_content = new_content.replace(&script, "");
        }
    }

    // --- Remove dns-prefetch link elements ---
    // Match both <link .../> and <link ...></link> formats
    let dns_prefetch_regex = regex::Regex::new(r#"<link[^>]*rel\s*=\s*["']dns-prefetch["'][^>]*/?>"#)
        .context("Failed to create dns-prefetch regex")?;

    let mut link_removals = Vec::new();

    for cap in dns_prefetch_regex.captures_iter(&new_content) {
        if let Some(full_match) = cap.get(0) {
            link_removals.push(full_match.as_str().to_string());
        }
    }

    if !link_removals.is_empty() {
        println!("   {}", format!("🔗 Eliminando {} elementos <link> dns-prefetch...", link_removals.len()).yellow().dimmed());
        for link in link_removals {
            new_content = new_content.replace(&link, "");
        }
    }

    // --- Remove Google Tag Manager link elements ---
    // Match <link> elements with href containing googletagmanager.com
    // This includes preconnect, dns-prefetch, and any other link types to GTM
    let gtm_link_regex = regex::Regex::new(r#"<link[^>]*href\s*=\s*["']https?://[^"']*googletagmanager\.com[^"']*["'][^>]*/?>"#)
        .context("Failed to create Google Tag Manager link regex")?;

    let mut gtm_link_removals = Vec::new();

    for cap in gtm_link_regex.captures_iter(&new_content) {
        if let Some(full_match) = cap.get(0) {
            gtm_link_removals.push(full_match.as_str().to_string());
        }
    }

    if !gtm_link_removals.is_empty() {
        println!("   {}", format!("📊 Eliminando {} elementos <link> de Google Tag Manager...", gtm_link_removals.len()).yellow().dimmed());
        for link in gtm_link_removals {
            new_content = new_content.replace(&link, "");
        }
    }

    // --- Remove create account form elements ---
    let create_account_regex = regex::Regex::new(r#"<v-create-account-form[^>]*>.*?</v-create-account-form>"#)
        .context("Failed to create v-create-account-form regex")?;

    let mut form_removals = Vec::new();

    for cap in create_account_regex.captures_iter(&new_content) {
        if let Some(full_match) = cap.get(0) {
            form_removals.push(full_match.as_str().to_string());
        }
    }

    if !form_removals.is_empty() {
        println!("   {}", format!("📝 Eliminando {} elementos <v-create-account-form>...", form_removals.len()).yellow().dimmed());
        for form in form_removals {
            new_content = new_content.replace(&form, "");
        }
    }

    // --- Remove cookie banner menu items ---
    // Match <li> elements containing <a> with onclick="showFullCookieBanner();"
    let cookie_banner_li_regex = regex::Regex::new(r#"<li[^>]*>.*?<a[^>]*onclick\s*=\s*["']showFullCookieBanner\(\);["'][^>]*>.*?</a>.*?</li>"#)
        .context("Failed to create cookie banner li regex")?;

    let mut li_removals = Vec::new();

    for cap in cookie_banner_li_regex.captures_iter(&new_content) {
        if let Some(full_match) = cap.get(0) {
            li_removals.push(full_match.as_str().to_string());
        }
    }

    if !li_removals.is_empty() {
        println!("   {}", format!("🍪 Eliminando {} elementos <li> con showFullCookieBanner...", li_removals.len()).yellow().dimmed());
        for li in li_removals {
            new_content = new_content.replace(&li, "");
        }
    }

    // --- Remove adBlock alert if blocks ---
    // Match: if (!getCookieAdvanced('adBlockAlertHidden')) { ... }
    let adblock_if_regex = regex::Regex::new(r#"if\s*\(\s*!getCookieAdvanced\s*\(\s*['"]adBlockAlertHidden['"]\s*\)\s*\)\s*\{[^{}]*\}"#)
        .context("Failed to create adblock if regex")?;

    let mut if_removals = Vec::new();

    for cap in adblock_if_regex.captures_iter(&new_content) {
        if let Some(full_match) = cap.get(0) {
            if_removals.push(full_match.as_str().to_string());
        }
    }

    if !if_removals.is_empty() {
        println!("   {}", format!("🚫 Eliminando {} bloques if de adBlock alert...", if_removals.len()).yellow().dimmed());
        for if_block in if_removals {
            new_content = new_content.replace(&if_block, "");
        }
    }

    // --- Remove holiday promo if-else blocks ---
    // Match: if (typeof page_params.holiday_promo_prem === 'undefined') { ... } else { ... }
    let holiday_promo_regex = regex::Regex::new(
        r#"if\s*\(\s*typeof\s+page_params\.holiday_promo_prem\s*===\s*['"]undefined['"]\s*\)\s*\{[^{}]*\}\s*else\s*\{[^{}]*\}"#
    ).context("Failed to create holiday promo regex")?;

    let mut promo_removals = Vec::new();

    for cap in holiday_promo_regex.captures_iter(&new_content) {
        if let Some(full_match) = cap.get(0) {
            promo_removals.push(full_match.as_str().to_string());
        }
    }

    if !promo_removals.is_empty() {
        println!("   {}", format!("🎁 Eliminando {} bloques if-else de holiday_promo_prem...", promo_removals.len()).yellow().dimmed());
        for promo_block in promo_removals {
            new_content = new_content.replace(&promo_block, "");
        }
    }

    // --- Download CDN Resources from JS Variables ---
    // Detect patterns like: playlistJs = "https://cdn.example.com/file.m3u8"
    let js_var_cdn_regex = regex::Regex::new(r#"(\w+)\s*=\s*["']((https?://[^"']*cdn[^"']*))["']"#)
        .context("Failed to create JS variable CDN regex")?;

    let mut cdn_replacements = Vec::new();

    for cap in js_var_cdn_regex.captures_iter(&new_content) {
        if let (Some(full_match), Some(var_name), Some(cdn_url)) = (cap.get(0), cap.get(1), cap.get(2)) {
            let var_name_str = var_name.as_str();
            let cdn_url_str = cdn_url.as_str();
            let full_match_str = full_match.as_str();

            // Extract safe filename from URL
            let file_name = extract_filename_from_url(cdn_url_str);

            // Try to find or download the resource
            let local_assets_dir = parent.join("assets");
            fs::create_dir_all(&local_assets_dir)?;

            let local_path = local_assets_dir.join(&file_name);
            let relative_path = format!("{}{}", assets_rel_path, file_name);

            // Download if needed
            if !local_path.exists() {
                println!("   {} {} -> {}",
                    "📦 Descargando recurso CDN:".cyan().dimmed(),
                    var_name_str.yellow(),
                    file_name.dimmed()
                );

                match download_resource(cdn_url_str, &local_path) {
                    Ok(_) => {
                        // Create replacement with local path
                        let replacement = format!(r#"{} = "{}""#, var_name_str, relative_path);
                        cdn_replacements.push((full_match_str.to_string(), replacement));
                    }
                    Err(e) => {
                        log::warn!("Failed to download CDN resource {}: {}", cdn_url_str, e);
                        println!("   {} {}",
                            "⚠️  Error descargando:".yellow(),
                            e.to_string().dimmed()
                        );
                    }
                }
            } else {
                // File already exists, just update the reference
                let replacement = format!(r#"{} = "{}""#, var_name_str, relative_path);
                cdn_replacements.push((full_match_str.to_string(), replacement));
            }
        }
    }

    // Apply CDN replacements
    for (old_str, new_str) in cdn_replacements {
        new_content = new_content.replace(&old_str, &new_str);
    }

    // --- Download Any CDN URLs in HTML (Generic CDN Detection) ---
    // Detect any https:// URL that contains "cdn" in the domain
    // Pattern matches: "https://anything-with-cdn-in-domain/path"
    let generic_cdn_regex = regex::Regex::new(r#"["'](https://[^/"']*cdn[^/"']*[^"']*)["']"#)
        .context("Failed to create generic CDN regex")?;

    let mut generic_cdn_replacements = Vec::new();

    for cap in generic_cdn_regex.captures_iter(&new_content) {
        if let Some(cdn_url_match) = cap.get(1) {
            let cdn_url_str = cdn_url_match.as_str();

            // Skip if this is a Google Fonts URL (already filtered)
            if cdn_url_str.contains("fonts.googleapis.com")
                || cdn_url_str.contains("fonts.gstatic.com") {
                continue;
            }

            // Extract safe filename from URL
            let file_name = extract_filename_from_url(cdn_url_str);

            // Try to find or download the resource
            let local_assets_dir = parent.join("assets");
            fs::create_dir_all(&local_assets_dir)?;

            let local_path = local_assets_dir.join(&file_name);
            let relative_path = format!("{}{}", assets_rel_path, file_name);

            // Download if needed
            if !local_path.exists() {
                println!("   {} {}",
                    "🌐 Descargando recurso CDN genérico:".cyan().dimmed(),
                    file_name.dimmed()
                );

                match download_resource(cdn_url_str, &local_path) {
                    Ok(_) => {
                        // Create replacement with local path
                        generic_cdn_replacements.push((cdn_url_str.to_string(), relative_path.clone()));
                    }
                    Err(e) => {
                        log::warn!("Failed to download generic CDN resource {}: {}", cdn_url_str, e);
                        println!("   {} {}",
                            "⚠️  Error descargando:".yellow(),
                            e.to_string().dimmed()
                        );
                    }
                }
            } else {
                // File already exists, just update the reference
                generic_cdn_replacements.push((cdn_url_str.to_string(), relative_path));
            }
        }
    }

    // Apply generic CDN replacements
    for (old_url, new_path) in generic_cdn_replacements {
        new_content = new_content.replace(&old_url, &new_path);
    }

    // --- Fix preventAutoplayForAVModal variable ---
    // Replace "preventAutoplayForAVModal = <anything>;" with "preventAutoplayForAVModal = true;"
    let autoplay_regex = regex::Regex::new(r"preventAutoplayForAVModal\s*=\s*[^;]+;")
        .context("Failed to create preventAutoplayForAVModal regex")?;

    if autoplay_regex.is_match(&new_content) {
        println!("   {}", "🎬 Ajustando preventAutoplayForAVModal = true...".cyan().dimmed());
        new_content = autoplay_regex.replace_all(&new_content, "preventAutoplayForAVModal = true;").to_string();
    }

    // --- Fix embedCode with improperly escaped quotes in iframes ---
    // Pattern: "embedCode":"<iframe src="..." ...>"
    // Should be: "embedCode":"<iframe src=\"...\" ...>"
    let embed_code_regex = regex::Regex::new(r#""embedCode"\s*:\s*"(<iframe[^>]*>(?:</iframe>)?)"#)
        .context("Failed to create embedCode regex")?;

    let mut embed_replacements = Vec::new();

    for cap in embed_code_regex.captures_iter(&new_content) {
        if let (Some(full_match), Some(iframe_content)) = (cap.get(0), cap.get(1)) {
            let original_iframe = iframe_content.as_str();
            let full_match_str = full_match.as_str();

            // Escape all quotes inside the iframe HTML
            // Replace " with \" and / with \/ for proper JSON escaping
            let escaped_iframe = original_iframe
                .replace('\\', r"\\")  // Escape backslashes first
                .replace('"', r#"\""#) // Escape double quotes
                .replace('/', r"\/");  // Escape forward slashes for JSON

            // Create the corrected embedCode entry
            let corrected = format!(r#""embedCode":"{}""#, escaped_iframe);

            // Only add if it's different (to avoid unnecessary replacements)
            if full_match_str != corrected {
                embed_replacements.push((full_match_str.to_string(), corrected));
            }
        }
    }

    if !embed_replacements.is_empty() {
        println!("   {}", format!("🔧 Corrigiendo {} embedCode con comillas mal escapadas...", embed_replacements.len()).cyan().dimmed());
        for (old, new) in embed_replacements {
            new_content = new_content.replace(&old, &new);
        }
    }

    // --- Fix rel=nofollow without quotes ---
    // Pattern: rel=nofollow (without quotes)
    // Should be: rel="nofollow"
    let rel_nofollow_regex = regex::Regex::new(r#"\brel=nofollow\b"#)
        .context("Failed to create rel=nofollow regex")?;

    if rel_nofollow_regex.is_match(&new_content) {
        let count = rel_nofollow_regex.find_iter(&new_content).count();
        println!("   {}", format!("🔗 Corrigiendo {} atributos rel=nofollow sin comillas...", count).cyan().dimmed());
        new_content = rel_nofollow_regex.replace_all(&new_content, r#"rel="nofollow""#).to_string();
    }

    // --- Remove Google Tag Manager iframes ---
    // Pattern: <iframe src="https://www.googletagmanager.com/..." ...></iframe>
    // These are tracking iframes that are not needed for offline viewing
    let gtm_iframe_regex = regex::Regex::new(r#"<iframe[^>]*src\s*=\s*["']https?://www\.googletagmanager\.com/[^"']*["'][^>]*>.*?</iframe>"#)
        .context("Failed to create Google Tag Manager iframe regex")?;

    let mut gtm_iframe_removals = Vec::new();

    for cap in gtm_iframe_regex.captures_iter(&new_content) {
        if let Some(full_match) = cap.get(0) {
            gtm_iframe_removals.push(full_match.as_str().to_string());
        }
    }

    if !gtm_iframe_removals.is_empty() {
        println!("   {}", format!("📊 Eliminando {} iframes de Google Tag Manager...", gtm_iframe_removals.len()).yellow().dimmed());
        for iframe in gtm_iframe_removals {
            new_content = new_content.replace(&iframe, "");
        }
    }

    fs::write(file_path, new_content)?;

    Ok(())
}

