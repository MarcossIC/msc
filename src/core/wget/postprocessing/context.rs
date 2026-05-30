use std::path::{Path, PathBuf};
use url::Url;

use crate::core::Blacklist;

use super::downloader::{Downloader, HTTP_DOWNLOADER};

/// A single URL replacement to apply to the HTML content.
pub struct Replacement {
    pub target: String,
    pub replacement: String,
    /// Images require special tag-aware replacement logic.
    pub is_image: bool,
}

/// Shared state threaded through all processing phases.
///
/// Created once per file, mutated by each phase in sequence.
pub struct ProcessingContext<'a> {
    // Immutable references shared across phases
    pub file_path: &'a Path,
    pub base_dir: &'a Path,
    pub base_url: &'a Url,
    pub blacklist: &'a Blacklist,

    // Derived paths (computed once at init)
    pub parent_dir: PathBuf,
    pub assets_rel_path: &'static str,

    // Mutable content flowing through the pipeline
    pub content: String,
    pub replacements: Vec<Replacement>,

    // Resource downloader (injectable for offline testing)
    pub downloader: &'a dyn Downloader,
}

impl<'a> ProcessingContext<'a> {
    /// Create a context backed by the production HTTP downloader.
    pub fn new(
        file_path: &'a Path,
        base_dir: &'a Path,
        base_url: &'a Url,
        blacklist: &'a Blacklist,
        content: String,
    ) -> Self {
        Self::with_downloader(
            file_path,
            base_dir,
            base_url,
            blacklist,
            content,
            &HTTP_DOWNLOADER,
        )
    }

    /// Create a context with an explicit downloader (used by tests to avoid network).
    pub fn with_downloader(
        file_path: &'a Path,
        base_dir: &'a Path,
        base_url: &'a Url,
        blacklist: &'a Blacklist,
        content: String,
        downloader: &'a dyn Downloader,
    ) -> Self {
        let parent_dir = file_path.parent().unwrap_or(base_dir).to_path_buf();
        Self {
            file_path,
            base_dir,
            base_url,
            blacklist,
            parent_dir,
            assets_rel_path: "assets/",
            content,
            replacements: Vec::new(),
            downloader,
        }
    }
}
