// File scanner for listing directory contents with Git integration

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use crate::git::{GitStatus, load_git_status, load_gitignore};
use crate::platform::is_hidden;
use ignore::gitignore::Gitignore;

/// Scanner for listing files with detailed information
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
    /// Create a new FileScanner for the given path
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

    /// Get the base path being scanned
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the Git status map
    pub fn git_status(&self) -> &Option<HashMap<String, GitStatus>> {
        &self.git_status
    }

    /// Get the Gitignore configuration
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

            // Skip hidden files if not requested
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
        entries.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
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
