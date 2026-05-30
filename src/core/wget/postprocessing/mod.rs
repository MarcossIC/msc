//! HTML post-processing pipeline for offline viewing.
//!
//! Transforms downloaded HTML files for offline use by:
//! - Downloading external resources (images, CSS, JS) to a local `assets/` directory
//! - Rewriting URLs to point to local copies
//! - Removing scripts that break offline viewing (tracking, redirects, cookies)
//! - Applying site-specific processing via the `SiteProcessor` trait
//!
//! # Architecture
//!
//! ```text
//! PostProcessor::process_file()
//!   ├─ phases::discover_and_download_resources()
//!   ├─ phases::convert_absolute_paths()
//!   ├─ phases::apply_replacements()
//!   ├─ SiteProcessor::process()  ← extensibility hook
//!   ├─ sanitizer::remove_dangerous_scripts()
//!   ├─ sanitizer::apply_sanitize_rules()
//!   ├─ phases::extract_cdn_resources()
//!   ├─ phases::apply_quirk_fixes()
//!   └─ write file
//! ```
//!
//! # Adding site-specific processing
//!
//! 1. Create a struct implementing `SiteProcessor` in `sites/`
//! 2. Register it in `PostProcessor::new()` in `pipeline.rs`
//!
//! See `sites/ts_reader.rs` for a complete example.

pub mod context;
mod phases;
pub mod pipeline;
pub mod sanitizer;
pub mod site_processor;
mod sites;

pub use pipeline::PostProcessor;

/// Backward-compatible entry point.
///
/// Prefer `PostProcessor::new(blacklist).process_file()` for batch processing
/// to avoid reloading the blacklist for every file.
pub fn process_html_file_complete(
    file_path: &std::path::PathBuf,
    base_dir: &std::path::PathBuf,
    base_url: &url::Url,
) -> anyhow::Result<()> {
    let blacklist = crate::core::validation::load_default_blacklist()
        .unwrap_or_else(|_| crate::core::Blacklist::new());
    let processor = PostProcessor::new(blacklist);
    processor.process_file(file_path, base_dir, base_url)
}
