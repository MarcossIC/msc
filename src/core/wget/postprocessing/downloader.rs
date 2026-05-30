//! Download abstraction for the post-processing pipeline.
//!
//! Phases that fetch remote resources go through the [`Downloader`] trait
//! instead of calling the HTTP client directly. Production code uses
//! [`HttpDownloader`]; tests inject a fake so the download path can be
//! exercised without touching the network.

use anyhow::Result;
use std::path::Path;

/// Abstraction over fetching a remote resource into a local file.
pub trait Downloader {
    /// Download `url` and write the bytes to `dest`.
    fn download(&self, url: &str, dest: &Path) -> Result<()>;
}

/// Production downloader backed by the blocking HTTP client in `wget_utils`.
pub struct HttpDownloader;

impl Downloader for HttpDownloader {
    fn download(&self, url: &str, dest: &Path) -> Result<()> {
        crate::core::wget::wget_utils::download_resource(url, &dest.to_path_buf())
    }
}

/// Shared zero-sized instance used as the default in `ProcessingContext::new`.
pub static HTTP_DOWNLOADER: HttpDownloader = HttpDownloader;

/// Test downloader that simulates a download by writing placeholder bytes,
/// or fails on demand to exercise the error path. No network involved.
#[cfg(test)]
pub struct FakeDownloader {
    pub fail: bool,
}

#[cfg(test)]
impl Downloader for FakeDownloader {
    fn download(&self, _url: &str, dest: &Path) -> Result<()> {
        if self.fail {
            anyhow::bail!("fake network error");
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(dest, b"fake-bytes")?;
        Ok(())
    }
}
