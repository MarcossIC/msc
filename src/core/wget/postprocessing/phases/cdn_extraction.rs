use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;

use crate::core::wget::wget_utils::{download_resource, extract_filename_from_url};

use super::super::context::ProcessingContext;

/// Phase 9: Download CDN resources referenced in JS variables and HTML attributes.
///
/// Detects two patterns:
/// 1. JS variable assignments: `playlistJs = "https://cdn.example.com/file.m3u8"`
/// 2. Generic CDN URLs: any `https://` URL with "cdn" in the domain
pub fn extract_cdn_resources(ctx: &mut ProcessingContext) -> Result<()> {
    extract_js_variable_cdn(ctx)?;
    extract_generic_cdn(ctx)?;
    Ok(())
}

/// Detect `varName = "https://...cdn.../path"` patterns and download.
fn extract_js_variable_cdn(ctx: &mut ProcessingContext) -> Result<()> {
    let re = regex::Regex::new(r#"(\w+)\s*=\s*["']((https?://[^"']*cdn[^"']*))["']"#)
        .context("Failed to create JS variable CDN regex")?;

    let mut replacements = Vec::new();

    for cap in re.captures_iter(&ctx.content) {
        let (Some(full_match), Some(var_name), Some(cdn_url)) =
            (cap.get(0), cap.get(1), cap.get(2))
        else {
            continue;
        };

        let var_name_str = var_name.as_str();
        let cdn_url_str = cdn_url.as_str();
        let file_name = extract_filename_from_url(cdn_url_str);

        let local_assets_dir = ctx.parent_dir.join("assets");
        fs::create_dir_all(&local_assets_dir)?;
        let local_path = local_assets_dir.join(&file_name);
        let relative_path = format!("{}{}", ctx.assets_rel_path, file_name);

        if !local_path.exists() {
            println!(
                "   {} {} -> {}",
                "📦 Descargando recurso CDN:".cyan().dimmed(),
                var_name_str.yellow(),
                file_name.dimmed()
            );
            match download_resource(cdn_url_str, &local_path) {
                Ok(_) => {
                    let replacement = format!(r#"{} = "{}""#, var_name_str, relative_path);
                    replacements.push((full_match.as_str().to_string(), replacement));
                }
                Err(e) => {
                    log::warn!("Failed to download CDN resource {}: {}", cdn_url_str, e);
                    println!(
                        "   {} {}",
                        "⚠️  Error descargando:".yellow(),
                        e.to_string().dimmed()
                    );
                }
            }
        } else {
            let replacement = format!(r#"{} = "{}""#, var_name_str, relative_path);
            replacements.push((full_match.as_str().to_string(), replacement));
        }
    }

    for (old, new) in replacements {
        ctx.content = ctx.content.replace(&old, &new);
    }

    Ok(())
}

/// Detect any `"https://...cdn.../path"` URL in HTML/JS and download.
fn extract_generic_cdn(ctx: &mut ProcessingContext) -> Result<()> {
    let re = regex::Regex::new(r#"["'](https://[^/"']*cdn[^/"']*[^"']*)["']"#)
        .context("Failed to create generic CDN regex")?;

    let mut replacements = Vec::new();

    for cap in re.captures_iter(&ctx.content) {
        let Some(cdn_url_match) = cap.get(1) else {
            continue;
        };
        let cdn_url_str = cdn_url_match.as_str();

        // Skip Google Fonts
        if cdn_url_str.contains("fonts.googleapis.com")
            || cdn_url_str.contains("fonts.gstatic.com")
        {
            continue;
        }

        let file_name = extract_filename_from_url(cdn_url_str);
        let local_assets_dir = ctx.parent_dir.join("assets");
        fs::create_dir_all(&local_assets_dir)?;
        let local_path = local_assets_dir.join(&file_name);
        let relative_path = format!("{}{}", ctx.assets_rel_path, file_name);

        if !local_path.exists() {
            println!(
                "   {} {}",
                "🌐 Descargando recurso CDN genérico:".cyan().dimmed(),
                file_name.dimmed()
            );
            match download_resource(cdn_url_str, &local_path) {
                Ok(_) => {
                    replacements.push((cdn_url_str.to_string(), relative_path));
                }
                Err(e) => {
                    log::warn!("Failed to download generic CDN resource {}: {}", cdn_url_str, e);
                    println!(
                        "   {} {}",
                        "⚠️  Error descargando:".yellow(),
                        e.to_string().dimmed()
                    );
                }
            }
        } else {
            replacements.push((cdn_url_str.to_string(), relative_path));
        }
    }

    for (old, new) in replacements {
        ctx.content = ctx.content.replace(&old, &new);
    }

    Ok(())
}
