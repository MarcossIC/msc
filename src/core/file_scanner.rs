//! File scanner for listing directory contents with Git integration
//!
//! This module provides a `FileScanner` that can scan directories and
//! return detailed information about files, including Git status and
//! respecting .gitignore patterns.
//!
//! # Examples
//!
//! ```no_run
//! use msc::core::file_scanner::FileScanner;
//! use std::path::Path;
//!
//! let scanner = FileScanner::new(Path::new("."))?;
//! let entries = scanner.scan(false)?;
//!
//! for entry in entries {
//!     println!("{}: {} bytes", entry.name, entry.size);
//! }
//! # Ok::<(), anyhow::Error>(())
//! ```

use crate::git::{load_git_status, load_gitignore, GitStatus};
use crate::platform::is_hidden;
use anyhow::Result;
use ignore::gitignore::Gitignore;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Scanner for listing files with detailed information
///
/// The `FileScanner` integrates with Git to provide status information
/// and respects `.gitignore` patterns when scanning directories.
pub struct FileScanner {
    path: PathBuf,
    git_status: Option<HashMap<String, GitStatus>>,
    gitignore: Option<Gitignore>,
}

/// Entry from a directory scan
#[derive(Debug)]
pub struct ScanEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub metadata: fs::Metadata,
}

impl FileScanner {
    pub fn new(path: &Path) -> Result<Self> {
        log::debug!("Creating FileScanner for path: {:?}", path);

        let git_status = load_git_status(path);
        let gitignore = load_gitignore(path);

        Ok(Self {
            path: path.to_path_buf(),
            git_status,
            gitignore,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn git_status(&self) -> &Option<HashMap<String, GitStatus>> {
        &self.git_status
    }

    pub fn gitignore(&self) -> &Option<Gitignore> {
        &self.gitignore
    }

    /// Scan the directory and return entries
    ///
    /// # Arguments
    /// * `show_hidden` - If true, include hidden files in results
    pub fn scan(&self, show_hidden: bool) -> Result<Vec<ScanEntry>> {
        let mut entries = Vec::new();
        let dir_entries = fs::read_dir(&self.path)?;

        for entry in dir_entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();

            if !show_hidden && (file_name.starts_with('.') || is_hidden(&entry)) {
                continue;
            }

            if let Ok(metadata) = entry.metadata() {
                let is_dir = metadata.is_dir();
                let size = if is_dir { 0 } else { metadata.len() };

                entries.push(ScanEntry {
                    name: file_name,
                    path: entry.path(),
                    is_dir,
                    size,
                    metadata,
                });
            }
        }

        // Sort: directories first, then alphabetically
        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_scanner_current_dir() {
        let scanner = FileScanner::new(Path::new("."));
        assert!(scanner.is_ok());
    }

    #[test]
    fn test_scan_current_directory() {
        let scanner = FileScanner::new(Path::new(".")).unwrap();
        let entries = scanner.scan(false);
        // Should be able to scan current directory
        assert!(entries.is_ok());
    }
}
