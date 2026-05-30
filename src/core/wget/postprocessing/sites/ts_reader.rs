use anyhow::{Context, Result};
use std::fs;
use url::Url;

use crate::core::wget::wget_utils::{
    calculate_possible_local_paths, download_resource, extract_filename_from_url, is_local_path,
};

use super::super::context::ProcessingContext;
use super::super::site_processor::SiteProcessor;

/// Site processor for pages using `ts_reader.run(JSON)` (manga/comic readers).
///
/// Extracts and localizes:
/// - Image URLs from `sources[*].images[]`
/// - Navigation links (`nextUrl`, `prevUrl`)
/// - Disables lazy loading (`lazyload = false`)
pub struct TsReaderProcessor;

impl SiteProcessor for TsReaderProcessor {
    fn name(&self) -> &str {
        "ts_reader (manga/comic reader)"
    }

    fn matches(&self, _base_url: &Url, content: &str) -> bool {
        content.contains("ts_reader.run(")
    }

    fn process(&self, ctx: &mut ProcessingContext) -> Result<()> {
        let re = regex::Regex::new(r#"ts_reader\.run\((.*)\);"#)
            .context("Failed to create ts_reader regex")?;

        let mut script_replacements = Vec::new();

        for cap in re.captures_iter(&ctx.content) {
            let Some(json_match) = cap.get(1) else {
                continue;
            };
            let json_str = json_match.as_str();
            let Ok(mut json_data) = serde_json::from_str::<serde_json::Value>(json_str) else {
                continue;
            };

            let mut modified = false;

            if let Some(obj) = json_data.as_object_mut() {
                // Disable lazy loading
                obj.insert("lazyload".to_string(), serde_json::Value::Bool(false));
                modified = true;

                // Localize nextUrl
                if let Some(new_val) = localize_nav_url(obj.get("nextUrl"), ctx) {
                    obj.insert("nextUrl".to_string(), new_val);
                    modified = true;
                }

                // Localize prevUrl
                if let Some(new_val) = localize_nav_url(obj.get("prevUrl"), ctx) {
                    obj.insert("prevUrl".to_string(), new_val);
                    modified = true;
                }
            }

            // Localize images in sources[].images[]
            if let Some(sources) = json_data.get_mut("sources").and_then(|s| s.as_array_mut()) {
                for source in sources {
                    if let Some(images) = source.get_mut("images").and_then(|i| i.as_array_mut()) {
                        for image_val in images {
                            if localize_image(image_val, ctx) {
                                modified = true;
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

        for (old_json, new_json) in script_replacements {
            ctx.content = ctx.content.replace(&old_json, &new_json);
        }

        Ok(())
    }
}

/// Resolve a navigation URL (nextUrl/prevUrl) to a local relative path.
fn localize_nav_url(
    url_val: Option<&serde_json::Value>,
    ctx: &ProcessingContext,
) -> Option<serde_json::Value> {
    let url_str = url_val?.as_str()?;

    let resolved_url = if url_str.starts_with("http://") || url_str.starts_with("https://") {
        Url::parse(url_str).ok()
    } else {
        ctx.base_url.join(url_str).ok()
    }?;

    if resolved_url.domain() != ctx.base_url.domain() {
        return None;
    }

    let possible_paths = calculate_possible_local_paths(&resolved_url, ctx.base_dir);
    let local_path = possible_paths.iter().find(|p| p.exists())?;
    let parent_file = ctx.file_path.parent().unwrap_or(ctx.base_dir);
    let relative = pathdiff::diff_paths(local_path, parent_file)?;
    let relative_str = relative.to_string_lossy().replace('\\', "/");

    Some(serde_json::Value::String(relative_str))
}

/// Download and localize a single image from the ts_reader JSON.
fn localize_image(image_val: &mut serde_json::Value, ctx: &ProcessingContext) -> bool {
    let Some(img_url) = image_val.as_str() else {
        return false;
    };

    if is_local_path(img_url) {
        return false;
    }

    let full_url = if img_url.starts_with("//") {
        format!("https:{}", img_url)
    } else if img_url.starts_with("http://") || img_url.starts_with("https://") {
        img_url.to_string()
    } else {
        match ctx.base_url.join(img_url) {
            Ok(resolved) => resolved.to_string(),
            Err(_) => return false,
        }
    };

    let file_name = extract_filename_from_url(&full_url);
    let local_assets_dir = ctx.parent_dir.join("assets");
    let global_assets_dir = ctx.base_dir.join("assets");

    let local_path = local_assets_dir.join(&file_name);
    let global_path = global_assets_dir.join(&file_name);

    let (final_path, replacement_path) = if local_path.exists() {
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
    } else if fs::create_dir_all(&local_assets_dir).is_ok() {
        (
            local_path,
            format!("{}{}", ctx.assets_rel_path, file_name),
        )
    } else {
        return false;
    };

    if !final_path.exists() {
        match download_resource(&full_url, &final_path) {
            Ok(_) => {
                *image_val = serde_json::Value::String(replacement_path);
                true
            }
            Err(e) => {
                log::warn!("Failed to download script image {}: {}", full_url, e);
                false
            }
        }
    } else {
        *image_val = serde_json::Value::String(replacement_path);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Blacklist;
    use tempfile::tempdir;

    #[test]
    fn matches_only_ts_reader_pages() {
        let p = TsReaderProcessor;
        let url = Url::parse("https://example.com/").unwrap();
        assert!(p.matches(&url, r#"<script>ts_reader.run({"sources":[]});</script>"#));
        assert!(!p.matches(&url, "<html><body>nada que ver</body></html>"));
    }

    // --- Capa 2: procesamiento con filesystem real, sin red ---
    // El asset se pre-crea, así `localize_image` toma el camino "ya existe".

    #[test]
    fn process_disables_lazyload_and_localizes_existing_image() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        let file_path = base.join("page.html");
        std::fs::create_dir_all(base.join("assets")).unwrap();
        std::fs::write(base.join("assets").join("img1.png"), b"img").unwrap();

        let blacklist = Blacklist::new();
        let base_url = Url::parse("https://example.com/").unwrap();
        let content =
            r#"<script>ts_reader.run({"sources":[{"images":["http://cdn/img1.png"]}]});</script>"#;
        let mut ctx =
            ProcessingContext::new(&file_path, base, &base_url, &blacklist, content.to_string());

        TsReaderProcessor.process(&mut ctx).unwrap();

        assert!(ctx.content.contains(r#""lazyload":false"#), "got: {}", ctx.content);
        assert!(ctx.content.contains("assets/img1.png"), "got: {}", ctx.content);
        assert!(
            !ctx.content.contains("http://cdn/img1.png"),
            "la URL remota debe quedar localizada, got: {}",
            ctx.content
        );
    }

    #[test]
    fn localize_nav_url_resolves_existing_local_page() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        let site = base.join("example.com");
        std::fs::create_dir_all(site.join("chapter-2")).unwrap();
        std::fs::write(site.join("chapter-2").join("index.html"), b"<html></html>").unwrap();
        let file_path = site.join("page.html");

        let blacklist = Blacklist::new();
        let base_url = Url::parse("https://example.com/").unwrap();
        let ctx = ProcessingContext::new(&file_path, base, &base_url, &blacklist, String::new());

        let nav = serde_json::Value::String("https://example.com/chapter-2/".to_string());
        let res = localize_nav_url(Some(&nav), &ctx);
        assert_eq!(
            res,
            Some(serde_json::Value::String("chapter-2/index.html".to_string()))
        );
    }
}
