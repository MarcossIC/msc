use anyhow::Result;
use colored::Colorize;
use std::fs;

use crate::core::validation::validate_url_not_blacklisted;
use crate::core::wget::wget_utils::{
    calculate_local_path_for_url, download_resource, extract_filename_from_url,
    is_placeholder_image,
};

use super::super::context::{ProcessingContext, Replacement};

/// Resolve a URL, download the resource if needed, and return its local relative path.
///
/// This helper deduplicates the resource download logic that was previously
/// copy-pasted for `src`/`href` and `srcset` processing.
fn resolve_and_download(
    ctx: &ProcessingContext,
    url_str: &str,
    log_prefix: &str,
) -> Result<Option<String>> {
    let full_url = if url_str.starts_with("//") {
        format!("https:{}", url_str)
    } else {
        url_str.to_string()
    };

    // Check blacklist
    if let Err(e) = validate_url_not_blacklisted(&full_url, ctx.blacklist) {
        println!(
            "   {} {} - {}",
            format!("🚫 URL bloqueada{}:", log_prefix).red().dimmed(),
            full_url.dimmed(),
            e.to_string().yellow()
        );
        return Ok(None);
    }

    let file_name = extract_filename_from_url(&full_url);
    let local_assets_dir = ctx.parent_dir.join("assets");
    let global_assets_dir = ctx.base_dir.join("assets");

    let local_path = local_assets_dir.join(&file_name);
    let global_path = global_assets_dir.join(&file_name);

    let (final_path, relative_path) = if local_path.exists() {
        (local_path, format!("{}{}", ctx.assets_rel_path, file_name))
    } else if global_path.exists() {
        let parent_to_base =
            pathdiff::diff_paths(ctx.base_dir, &ctx.parent_dir).unwrap_or_default();
        let mut rel_to_global = String::new();
        for _ in 0..parent_to_base.components().count() {
            rel_to_global.push_str("../");
        }
        rel_to_global.push_str("assets/");
        rel_to_global.push_str(&file_name);
        (global_path, rel_to_global)
    } else {
        fs::create_dir_all(&local_assets_dir)?;
        (local_path, format!("{}{}", ctx.assets_rel_path, file_name))
    };

    if !final_path.exists() {
        match download_resource(&full_url, &final_path) {
            Ok(_) => {
                println!(
                    "   {} {} -> {}",
                    format!("✓ Descargado{}:", log_prefix).green().dimmed(),
                    full_url.dimmed(),
                    file_name.cyan()
                );
            }
            Err(e) => {
                println!(
                    "   {} {} -> Error: {}",
                    format!("✗ Error descargando{}:", log_prefix).red().dimmed(),
                    full_url.dimmed(),
                    e.to_string().yellow()
                );
                log::warn!("Failed to download {}: {}", full_url, e);
                return Ok(None);
            }
        }
    }

    Ok(Some(relative_path))
}

fn should_ignore_url(url_str: &str) -> bool {
    url_str.contains("fonts.googleapis.com")
        || url_str.contains("fonts.gstatic.com")
        || url_str.contains("googletagmanager.com")
        || url_str.ends_with(".php")
        || url_str.contains("xmlrpc.php")
}

fn is_external_url(url_str: &str) -> bool {
    url_str.starts_with("http://")
        || url_str.starts_with("https://")
        || url_str.starts_with("//")
}

/// Phase 2: Traverse DOM elements, discover external resources, download them locally.
///
/// Handles `<img>`, `<script>`, `<link>`, `<a>`, `<iframe>` elements including:
/// - Lazy-loading attributes (data-src, data-original, etc.)
/// - srcset multi-resolution images
/// - `<a>` link rewriting to local paths
/// - Blacklist filtering
pub fn discover_and_download_resources(ctx: &mut ProcessingContext) -> Result<()> {
    let document = scraper::Html::parse_document(&ctx.content);
    let selector = scraper::Selector::parse("img, script, link, a, iframe")
        .map_err(|e| anyhow::anyhow!("Failed to create selector: {:?}", e))?;

    for element in document.select(&selector) {
        let tag_name = element.value().name();

        let attr_name = match tag_name {
            "a" | "link" => "href",
            "img" | "script" | "iframe" => "src",
            _ => continue,
        };

        let mut url_val = element.value().attr(attr_name);
        let mut srcset_urls: Vec<String> = Vec::new();

        // Handle srcset and lazy-loading for images
        if tag_name == "img" {
            if let Some(srcset_val) = element.value().attr("srcset") {
                if !srcset_val.is_empty() {
                    for srcset_entry in srcset_val.split(',') {
                        let trimmed = srcset_entry.trim();
                        if let Some(url_part) = trimmed.split_whitespace().next() {
                            if !url_part.is_empty() && is_external_url(url_part) {
                                srcset_urls.push(url_part.to_string());
                            }
                        }
                    }
                }
            }

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
            if should_ignore_url(url_str) {
                continue;
            }

            // Handle <a> links — rewrite to local relative paths
            if tag_name == "a" {
                process_anchor_link(ctx, url_str);
                continue;
            }

            // Download external resources
            if is_external_url(url_str) {
                if tag_name == "img" {
                    println!(
                        "   {} {} ({})",
                        "🔍 Detectada imagen externa:".blue().dimmed(),
                        url_str.dimmed(),
                        tag_name.yellow()
                    );
                }

                if let Ok(Some(relative_path)) = resolve_and_download(ctx, url_str, "") {
                    // For images, also replace placeholder src if present
                    if tag_name == "img" {
                        if let Some(original_src) = element.value().attr("src") {
                            if is_placeholder_image(original_src) {
                                ctx.replacements.push(Replacement {
                                    target: original_src.to_string(),
                                    replacement: relative_path.clone(),
                                    is_image: true,
                                });
                            }
                        }
                        ctx.replacements.push(Replacement {
                            target: url_str.to_string(),
                            replacement: relative_path,
                            is_image: true,
                        });
                    } else {
                        ctx.replacements.push(Replacement {
                            target: url_str.to_string(),
                            replacement: relative_path,
                            is_image: false,
                        });
                    }
                }
            }
        }

        // Process srcset URLs
        for srcset_url in &srcset_urls {
            if should_ignore_url(srcset_url) {
                continue;
            }
            if is_external_url(srcset_url) {
                println!(
                    "   {} {} (srcset)",
                    "🔍 Detectada imagen en srcset:".blue().dimmed(),
                    srcset_url.dimmed()
                );
                if let Ok(Some(relative_path)) = resolve_and_download(ctx, srcset_url, " (srcset)")
                {
                    ctx.replacements.push(Replacement {
                        target: srcset_url.clone(),
                        replacement: relative_path,
                        is_image: true,
                    });
                }
            }
        }
    }

    Ok(())
}

/// Rewrite `<a>` links to local relative paths if the target file exists.
fn process_anchor_link(ctx: &mut ProcessingContext, url_str: &str) {
    let Ok(resolved_url) = ctx.base_url.join(url_str) else {
        return;
    };
    if resolved_url.domain() != ctx.base_url.domain() {
        return;
    }
    let Some(local_link_path) = calculate_local_path_for_url(&resolved_url, ctx.base_dir) else {
        return;
    };
    if !local_link_path.exists() {
        return;
    }
    let Some(relative_link) = pathdiff::diff_paths(&local_link_path, &ctx.parent_dir) else {
        return;
    };
    let relative_link_str = relative_link.to_string_lossy().replace('\\', "/");
    ctx.replacements.push(Replacement {
        target: url_str.to_string(),
        replacement: relative_link_str,
        is_image: false,
    });
}
