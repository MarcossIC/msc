use anyhow::Result;
use url::Url;

use super::context::ProcessingContext;
use super::sanitizer::SanitizeRule;

/// Trait for site-specific post-processing logic.
///
/// Implementations handle parsings unique to a particular website or family of sites.
/// The pipeline auto-detects which processor applies via `matches()` and invokes it.
///
/// # Adding a new site processor
///
/// 1. Create a new file in `postprocessing/sites/`
/// 2. Implement `SiteProcessor`
/// 3. Register it in `PostProcessor::new()` in `pipeline.rs`
///
/// ```ignore
/// pub struct MySiteProcessor;
///
/// impl SiteProcessor for MySiteProcessor {
///     fn name(&self) -> &str { "MySite" }
///     fn matches(&self, _url: &Url, content: &str) -> bool {
///         content.contains("mysite_reader.init(")
///     }
///     fn process(&self, ctx: &mut ProcessingContext) -> Result<()> {
///         // Site-specific extraction/download/rewriting logic
///         Ok(())
///     }
/// }
/// ```
pub trait SiteProcessor {
    /// Human-readable name for logging.
    fn name(&self) -> &str;

    /// Return true if this processor should handle the given URL/content.
    fn matches(&self, base_url: &Url, content: &str) -> bool;

    /// Additional sanitize rules specific to this site (appended to defaults).
    fn extra_sanitize_rules(&self) -> Vec<SanitizeRule> {
        vec![]
    }

    /// Site-specific processing. Called after resource discovery and URL replacement,
    /// but before generic sanitization.
    fn process(&self, ctx: &mut ProcessingContext) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}
