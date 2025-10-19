//! Temporary files cleaner for system directories
//!
//! This module provides functionality to scan and clean temporary files
//! from system temporary directories.
//!
//! # Examples
//!
//! ```no_run
//! use msc::core::cleaner::TempCleaner;
//!
//! let cleaner = TempCleaner::new()?;
//! 
//! // Scan to get statistics
//! let stats = cleaner.scan();
//! println!("Found {} files ({} bytes)", stats.total_files, stats.total_size);
//!
//! // Clean with dry-run
//! let stats = cleaner.clean(true, |processed, total| {
//!     println!("Progress: {}/{}", processed, total);
//! })?;
//! # Ok::<(), anyhow::Error>(())
//! ```

use anyhow::Result;
use std::path::Path;
use std::fs;
use crate::platform::get_temp_directories;

/// Temporary files cleaner
///
/// Scans and optionally deletes files from system temporary directories.
/// Supports both dry-run mode (simulation) and actual deletion with progress tracking.
pub struct TempCleaner {
    pub directories: Vec<String>,
}

/// Statistics from cleanup operations
#[derive(Debug, Default)]
pub struct CleanupStats {
    pub total_files: usize,
    pub total_size: u64,
    pub deleted_files: usize,
    pub deleted_size: u64,
    pub failed_files: usize,
}

impl TempCleaner {
    /// Create a new TempCleaner with system temporary directories
    pub fn new() -> Result<Self> {
        let directories = get_temp_directories();
        Ok(Self { directories })
    }

    /// Scan temporary directories and count files
    pub fn scan(&self) -> CleanupStats {
        let mut stats = CleanupStats::default();

        for temp_dir in &self.directories {
            count_files_recursive(Path::new(temp_dir), &mut stats.total_files, &mut stats.total_size);
        }

        stats
    }

    /// Clean temporary files with progress callback
    ///
    /// # Arguments
    /// * `dry_run` - If true, only simulate deletion without actually deleting
    /// * `on_progress` - Callback function called with (processed, total) for each file
    pub fn clean<F>(&self, dry_run: bool, on_progress: F) -> Result<CleanupStats>
    where
        F: Fn(usize, usize),
    {
        let stats = self.scan();

        let mut ctx = DeleteContext {
            processed: 0,
            total_files: stats.total_files,
            deleted_files: 0,
            deleted_size: 0,
            failed_files: 0,
            dry_run,
            on_progress: &on_progress,
        };

        for temp_dir in &self.directories {
            delete_files_recursive(Path::new(temp_dir), &mut ctx);
        }

        Ok(CleanupStats {
            total_files: stats.total_files,
            total_size: stats.total_size,
            deleted_files: ctx.deleted_files,
            deleted_size: ctx.deleted_size,
            failed_files: ctx.failed_files,
        })
    }
}

/// Count files recursively in a directory
fn count_files_recursive(dir: &Path, total_files: &mut usize, total_size: &mut u64) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    *total_files += 1;
                    *total_size += metadata.len();
                } else if metadata.is_dir() {
                    count_files_recursive(&entry.path(), total_files, total_size);
                }
            }
        }
    }
}

/// Context for delete operations
struct DeleteContext<'a, F>
where
    F: Fn(usize, usize),
{
    processed: usize,
    total_files: usize,
    deleted_files: usize,
    deleted_size: u64,
    failed_files: usize,
    dry_run: bool,
    on_progress: &'a F,
}

/// Delete files recursively with progress tracking
fn delete_files_recursive<F>(dir: &Path, ctx: &mut DeleteContext<F>)
where
    F: Fn(usize, usize),
{
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    ctx.processed += 1;
                    let file_path = entry.path();
                    let file_size = metadata.len();

                    (ctx.on_progress)(ctx.processed, ctx.total_files);

                    if ctx.dry_run {
                        ctx.deleted_files += 1;
                        ctx.deleted_size += file_size;
                    } else {
                        match fs::remove_file(&file_path) {
                            Ok(_) => {
                                ctx.deleted_files += 1;
                                ctx.deleted_size += file_size;
                            }
                            Err(_) => {
                                ctx.failed_files += 1;
                            }
                        }
                    }
                } else if metadata.is_dir() {
                    delete_files_recursive(&entry.path(), ctx);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_cleaner_new() {
        let cleaner = TempCleaner::new();
        assert!(cleaner.is_ok());
    }

    #[test]
    fn test_scan_returns_stats() {
        let cleaner = TempCleaner::new().unwrap();
        let stats = cleaner.scan();
        // Just verify we can scan without errors (total_files is always valid usize)
        let _ = stats.total_files;
    }
}
