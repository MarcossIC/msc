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
    pub fn process_file(
        &self,
        file_path: &Path,
        base_dir: &Path,
        base_url: &Url,
    ) -> Result<()> {
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

        let mut ctx = ProcessingContext::new(file_path, base_dir, base_url, &self.blacklist, content);

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
