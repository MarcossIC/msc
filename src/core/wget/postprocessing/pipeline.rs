use anyhow::Result;
use std::path::Path;
use url::Url;

use crate::core::Blacklist;

use super::context::ProcessingContext;
use super::phases;
use super::sanitizer;
use super::site_processor::SiteProcessor;
use super::sites;

/// HTML post-processor for offline viewing.
///
/// Holds shared state (blacklist, site processors) that is reused across
/// multiple files in a batch. Create once, call `process_file` per HTML file.
///
/// # Example
/// ```ignore
/// let processor = PostProcessor::new(blacklist);
/// for html_file in &html_files {
///     processor.process_file(html_file, &base_dir, &base_url)?;
/// }
/// ```
pub struct PostProcessor {
    blacklist: Blacklist,
    site_processors: Vec<Box<dyn SiteProcessor>>,
}

impl PostProcessor {
    /// Create a new post-processor with the given blacklist.
    ///
    /// To add site-specific processors, register them in this constructor.
    pub fn new(blacklist: Blacklist) -> Self {
        Self {
            blacklist,
            site_processors: vec![
                Box::new(sites::ts_reader::TsReaderProcessor),
                // Future site processors go here:
                // Box::new(sites::wordpress::WordPressProcessor),
                // Box::new(sites::medium::MediumProcessor),
            ],
        }
    }

    /// Process a single HTML file for offline viewing.
    ///
    /// Pipeline:
    /// 1. Discover and download external resources (img, css, js)
    /// 2. Convert absolute paths to relative
    /// 3. Apply URL replacements
    /// 4. Run site-specific processors (if matched)
    /// 5. Remove dangerous scripts (redirects, tracking, cookies)
    /// 6. Apply sanitize rules (dns-prefetch, GTM, forms, promos)
    /// 7. Extract CDN resources
    /// 8. Fix HTML/JS quirks
    pub fn process_file(&self, file_path: &Path, base_dir: &Path, base_url: &Url) -> Result<()> {
        let content = std::fs::read_to_string(file_path)?;

        // Log blacklist status on first file (non-empty blacklist)
        if !self.blacklist.is_empty() {
            use colored::Colorize;
            println!(
                "   {} {} dominios bloqueados",
                "🛡️  Blacklist cargada:".dimmed(),
                self.blacklist.len().to_string().cyan()
            );
        }

        let mut ctx =
            ProcessingContext::new(file_path, base_dir, base_url, &self.blacklist, content);

        // Phase 2: Resource discovery and downloading (critical — propagates errors)
        phases::discover_and_download_resources(&mut ctx)?;

        // Phase 3: Absolute path conversion (non-critical)
        if let Err(e) = phases::convert_absolute_paths(&mut ctx) {
            log::warn!("Absolute path conversion failed: {}", e);
        }

        // Phase 4: Apply URL replacements (critical)
        phases::apply_replacements(&mut ctx)?;

        // Site-specific processing (extensibility point)
        for processor in &self.site_processors {
            if processor.matches(base_url, &ctx.content) {
                if let Err(e) = processor.process(&mut ctx) {
                    log::warn!("Site processor '{}' failed: {}", processor.name(), e);
                }
            }
        }

        // Phase 6: Remove dangerous scripts (predicate-based)
        sanitizer::remove_dangerous_scripts(&mut ctx.content);

        // Phases 7-8, 10: Apply sanitize rules (data-driven)
        let mut rules = sanitizer::default_sanitize_rules();
        for processor in &self.site_processors {
            if processor.matches(base_url, &ctx.content) {
                rules.extend(processor.extra_sanitize_rules());
            }
        }
        sanitizer::apply_sanitize_rules(&mut ctx.content, &rules);

        // Phase 9: CDN resource extraction (non-critical)
        if let Err(e) = phases::extract_cdn_resources(&mut ctx) {
            log::warn!("CDN resource extraction failed: {}", e);
        }

        // Phase 10: Variable/quirk fixes (non-critical)
        if let Err(e) = phases::apply_quirk_fixes(&mut ctx) {
            log::warn!("Quirk fixes failed: {}", e);
        }

        // Phase 11: Write
        std::fs::write(file_path, &ctx.content)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// End-to-end characterization of the full pipeline over a single fixture
    /// that exercises several phases at once. Assets are pre-created so NO
    /// network access happens. Verified against the behavior of the legacy
    /// `wgetpostprocessing.rs` (commit 02719ef).
    #[test]
    fn process_file_applies_full_pipeline_offline() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        // Pre-create the asset so the external image resolves locally (no download).
        std::fs::create_dir_all(base.join("assets")).unwrap();
        std::fs::write(base.join("assets").join("photo.png"), b"img").unwrap();

        let html = r#"<!DOCTYPE html>
<html>
<head>
<link rel="dns-prefetch" href="//cdn.tracker.com">
<title>Test</title>
</head>
<body>
<img src="http://images.example.org/photo.png">
<a href="/local/page">enlace interno</a>
<a rel=nofollow href="http://external.com">externo</a>
<script>var redirectUrl = "http://spam.com"; window.location = redirectUrl;</script>
<script>console.log("contenido legitimo");</script>
<iframe src="https://www.googletagmanager.com/ns.html?id=GTM-XXX"></iframe>
<script>var x = 1; preventAutoplayForAVModal = false; var y = 2;</script>
</body>
</html>"#;

        let file_path = base.join("page.html");
        std::fs::write(&file_path, html).unwrap();

        let base_url = Url::parse("https://example.com/").unwrap();
        let processor = PostProcessor::new(Blacklist::new());
        processor.process_file(&file_path, base, &base_url).unwrap();

        let out = std::fs::read_to_string(&file_path).unwrap();

        // Resource discovery: external image localized to the pre-created asset.
        assert!(
            out.contains(r#"src="assets/photo.png""#),
            "img not localized:\n{}",
            out
        );
        // Absolute path conversion: "/local/page" -> "local/page" (depth 0).
        assert!(
            out.contains(r#"href="local/page""#),
            "absolute path not converted:\n{}",
            out
        );
        // Dangerous script removal: redirect gone, legit script kept.
        assert!(
            !out.contains("redirectUrl"),
            "redirect script not removed:\n{}",
            out
        );
        assert!(
            out.contains("contenido legitimo"),
            "legit script wrongly removed:\n{}",
            out
        );
        // Sanitize rules: dns-prefetch link and GTM iframe removed.
        assert!(
            !out.contains("dns-prefetch"),
            "dns-prefetch not removed:\n{}",
            out
        );
        assert!(
            !out.contains("googletagmanager"),
            "GTM iframe not removed:\n{}",
            out
        );
        // Quirk fix: autoplay forced to true.
        assert!(
            out.contains("preventAutoplayForAVModal = true;"),
            "autoplay not fixed:\n{}",
            out
        );
        assert!(!out.contains("= false"), "autoplay still false:\n{}", out);
        // Sanitize rule: bare rel=nofollow quoted.
        assert!(
            out.contains(r#"rel="nofollow""#),
            "nofollow not quoted:\n{}",
            out
        );
        assert!(
            !out.contains("rel=nofollow"),
            "bare nofollow remains:\n{}",
            out
        );
    }
}
