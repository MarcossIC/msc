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

use crate::core::Config;
use anyhow::Result;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// Temporary files cleaner
///
/// Scans and optionally deletes files from system temporary directories.
/// Supports both dry-run mode (simulation) and actual deletion with progress tracking.
/// Includes cooperative cancellation support via Ctrl+C.
pub struct TempCleaner {
    pub directories: Vec<String>,
    pub min_age: Option<Duration>,
    pub max_age: Option<Duration>,
    pub cancel_flag: Arc<AtomicBool>,
}

/// Statistics from cleanup operations
#[derive(Debug, Default)]
pub struct CleanupStats {
    pub total_files: usize,
    pub total_size: u64,
    pub deleted_files: usize,
    pub deleted_size: u64,
    pub failed_files: usize,
    pub inaccessible_dirs: Vec<String>,
    // Error tracking fields
    pub skipped_files: usize,
    pub permission_errors: usize,
    pub other_errors: usize,
    // Cancellation tracking
    pub was_cancelled: bool,
}

impl TempCleaner {
    /// Create a new TempCleaner with all active clean paths from config
    /// Paths are automatically synced with system defaults on each load
    /// By default, only deletes files older than 24 hours
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        // get_clean_paths() returns merged default + custom paths
        // default paths are synced dynamically on load
        let directories = config.get_clean_paths();
        Ok(Self {
            directories,
            min_age: Some(Duration::from_secs(24 * 3600)), // Default: 24 hours
            max_age: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Create a new TempCleaner with custom age filter
    pub fn with_age_filter(min_age_hours: Option<u64>) -> Result<Self> {
        let mut cleaner = Self::new()?;
        cleaner.min_age = min_age_hours.map(|h| Duration::from_secs(h * 3600));
        Ok(cleaner)
    }

    /// Set age filter for this cleaner instance
    pub fn set_age_filter(&mut self, min_age: Option<Duration>, max_age: Option<Duration>) {
        self.min_age = min_age;
        self.max_age = max_age;
    }

    /// Request cancellation of ongoing cleanup operation
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    /// Check if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    pub fn scan(&self) -> CleanupStats {
        let mut stats = CleanupStats::default();

        for temp_dir in &self.directories {
            let path = Path::new(temp_dir);

            // Check if directory exists and is accessible
            if !path.exists() || !path.is_dir() {
                stats.inaccessible_dirs.push(temp_dir.clone());
                continue;
            }

            count_files_recursive(path, &mut stats, self.min_age, self.max_age);
        }

        stats
    }

    /// Clean temporary files with progress callback
    ///
    /// # Arguments
    /// * `dry_run` - If true, only simulate deletion without actually deleting
    /// * `on_progress` - Callback function called with (processed, total) for each file
    pub fn clean<F>(&self, dry_run: bool, mut on_progress: F) -> Result<CleanupStats>
    where
        F: FnMut(usize, usize),
    {
        let stats = self.scan();

        let mut ctx = DeleteContext {
            processed: 0,
            total_files: stats.total_files,
            deleted_files: 0,
            deleted_size: 0,
            failed_files: 0,
            skipped_files: 0,
            permission_errors: 0,
            other_errors: 0,
            dry_run,
            on_progress: &mut on_progress,
            last_check: Instant::now(),
            check_interval: Duration::from_millis(100),
            throttle_delay: None,
        };

        for temp_dir in &self.directories {
            // Check for cancellation between directories
            if self.cancel_flag.load(Ordering::Relaxed) {
                break;
            }

            let path = Path::new(temp_dir);

            // Skip directories that don't exist or aren't accessible
            if !path.exists() || !path.is_dir() {
                continue;
            }

            delete_files_recursive_with_tracking(path, &mut ctx, self.min_age, self.max_age, &self.cancel_flag);
        }

        Ok(CleanupStats {
            total_files: stats.total_files,
            total_size: stats.total_size,
            deleted_files: ctx.deleted_files,
            deleted_size: ctx.deleted_size,
            failed_files: ctx.failed_files,
            inaccessible_dirs: stats.inaccessible_dirs,
            skipped_files: ctx.skipped_files,
            permission_errors: ctx.permission_errors,
            other_errors: ctx.other_errors,
            was_cancelled: self.cancel_flag.load(Ordering::Relaxed),
        })
    }
}

/// Check if a file should be deleted based on age criteria
fn should_delete_file(metadata: &fs::Metadata, min_age: Option<Duration>, max_age: Option<Duration>) -> bool {
    // Get file modification time
    let modified_time = match metadata.modified() {
        Ok(time) => time,
        Err(_) => return false, // Don't delete if we can't verify age
    };

    let now = SystemTime::now();
    let age = match now.duration_since(modified_time) {
        Ok(dur) => dur,
        Err(_) => return false, // File from future? Don't delete
    };

    // Check minimum age
    if let Some(min) = min_age {
        if age < min {
            return false; // Too new, don't delete
        }
    }

    // Check maximum age (if specified)
    if let Some(max) = max_age {
        if age > max {
            return false; // Too old, don't delete (use case: clean specific range)
        }
    }

    true
}

fn count_files_recursive(
    dir: &Path,
    stats: &mut CleanupStats,
    min_age: Option<Duration>,
    max_age: Option<Duration>,
) {
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry_result in entries {
                match entry_result {
                    Ok(entry) => {
                        match entry.metadata() {
                            Ok(metadata) => {
                                if metadata.is_file() {
                                    // Only count files that would be deleted based on age
                                    if should_delete_file(&metadata, min_age, max_age) {
                                        stats.total_files += 1;
                                        stats.total_size += metadata.len();
                                    } else {
                                        stats.skipped_files += 1;
                                    }
                                } else if metadata.is_dir() {
                                    count_files_recursive(&entry.path(), stats, min_age, max_age);
                                }
                            }
                            Err(e) => {
                                // Categorize metadata errors
                                if e.kind() == std::io::ErrorKind::PermissionDenied {
                                    stats.permission_errors += 1;
                                    log::warn!("Permission denied accessing metadata: {:?}", entry.path());
                                } else {
                                    stats.other_errors += 1;
                                    log::warn!("Error reading metadata for {:?}: {}", entry.path(), e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // Categorize entry read errors
                        if e.kind() == std::io::ErrorKind::PermissionDenied {
                            stats.permission_errors += 1;
                            log::warn!("Permission denied reading directory entry in {:?}", dir);
                        } else {
                            stats.skipped_files += 1;
                            log::warn!("Error reading directory entry in {:?}: {}", dir, e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            log::warn!("Cannot read directory {:?}: {}", dir, e);
            stats.other_errors += 1;
        }
    }
}

struct DeleteContext<'a, F>
where
    F: FnMut(usize, usize),
{
    processed: usize,
    total_files: usize,
    deleted_files: usize,
    deleted_size: u64,
    failed_files: usize,
    skipped_files: usize,
    permission_errors: usize,
    other_errors: usize,
    dry_run: bool,
    on_progress: &'a mut F,
    last_check: Instant,
    check_interval: Duration,
    throttle_delay: Option<Duration>,
}

fn delete_files_recursive_with_tracking<F>(
    dir: &Path,
    ctx: &mut DeleteContext<F>,
    min_age: Option<Duration>,
    max_age: Option<Duration>,
    cancel_flag: &Arc<AtomicBool>,
)
where
    F: FnMut(usize, usize),
{
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry_result in entries {
                // Periodic cancellation check with throttling
                let now = Instant::now();
                if now.duration_since(ctx.last_check) >= ctx.check_interval {
                    // Check for cancellation
                    if cancel_flag.load(Ordering::Relaxed) {
                        return; // Exit cleanly
                    }
                    ctx.last_check = now;

                    // Apply throttle if configured
                    if let Some(delay) = ctx.throttle_delay {
                        std::thread::sleep(delay);
                    }
                }

                match entry_result {
                    Ok(entry) => {
                        match entry.metadata() {
                            Ok(metadata) => {
                                if metadata.is_file() {
                                    ctx.processed += 1;
                                    let file_path = entry.path();
                                    let file_size = metadata.len();

                                    (ctx.on_progress)(ctx.processed, ctx.total_files);

                                    // Check if file should be deleted based on age
                                    if !should_delete_file(&metadata, min_age, max_age) {
                                        ctx.skipped_files += 1;
                                        continue;
                                    }

                                    if ctx.dry_run {
                                        ctx.deleted_files += 1;
                                        ctx.deleted_size += file_size;
                                    } else {
                                        match fs::remove_file(&file_path) {
                                            Ok(_) => {
                                                ctx.deleted_files += 1;
                                                ctx.deleted_size += file_size;
                                            }
                                            Err(e) => {
                                                ctx.failed_files += 1;
                                                if e.kind() == std::io::ErrorKind::PermissionDenied {
                                                    ctx.permission_errors += 1;
                                                    log::warn!("Permission denied deleting file: {:?}", file_path);
                                                } else {
                                                    ctx.other_errors += 1;
                                                    log::warn!("Error deleting file {:?}: {}", file_path, e);
                                                }
                                            }
                                        }
                                    }
                                } else if metadata.is_dir() {
                                    delete_files_recursive_with_tracking(&entry.path(), ctx, min_age, max_age, cancel_flag);
                                }
                            }
                            Err(e) => {
                                // Failed to get metadata
                                if e.kind() == std::io::ErrorKind::PermissionDenied {
                                    ctx.permission_errors += 1;
                                    log::warn!("Permission denied accessing metadata: {:?}", entry.path());
                                } else {
                                    ctx.other_errors += 1;
                                    log::warn!("Error reading metadata for {:?}: {}", entry.path(), e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // Failed to read directory entry
                        if e.kind() == std::io::ErrorKind::PermissionDenied {
                            ctx.permission_errors += 1;
                            log::warn!("Permission denied reading directory entry in {:?}", dir);
                        } else {
                            ctx.skipped_files += 1;
                            log::warn!("Error reading directory entry in {:?}: {}", dir, e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            log::warn!("Cannot read directory {:?}: {}", dir, e);
            ctx.other_errors += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use tempfile::TempDir;

    #[test]
    fn test_temp_cleaner_new() {
        let cleaner = TempCleaner::new();
        assert!(cleaner.is_ok());
    }

    #[test]
    fn test_scan_returns_stats() {
        let cleaner = TempCleaner::new().unwrap();
        let stats = cleaner.scan();
        let _ = stats.total_files;
    }

    #[test]
    fn test_cancel_flag_initial_state() {
        let cleaner = TempCleaner::new().unwrap();
        assert!(!cleaner.is_cancelled(), "Cancel flag should be false initially");
    }

    #[test]
    fn test_cancel_sets_flag() {
        let cleaner = TempCleaner::new().unwrap();
        assert!(!cleaner.is_cancelled());

        cleaner.cancel();

        assert!(cleaner.is_cancelled(), "Cancel flag should be true after cancel()");
    }

    #[test]
    fn test_cancel_flag_atomic() {
        let cleaner = TempCleaner::new().unwrap();
        let cancel_flag = cleaner.cancel_flag.clone();

        // Simulate another thread setting the flag
        cancel_flag.store(true, Ordering::Relaxed);

        assert!(cleaner.is_cancelled(), "Cancel flag should be visible across references");
    }

    #[test]
    fn test_cleanup_stats_tracks_cancellation() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // Create test files
        for i in 0..5 {
            std::fs::write(test_path.join(format!("file{}.txt", i)), "test content").unwrap();
        }

        let mut cleaner = TempCleaner::new().unwrap();
        cleaner.directories = vec![test_path.to_string_lossy().to_string()];
        cleaner.min_age = None; // Delete all files

        // Cancel immediately
        cleaner.cancel();

        let stats = cleaner.clean(false, |_, _| {}).unwrap();

        assert!(stats.was_cancelled, "Stats should indicate cancellation");
    }

    #[test]
    fn test_age_filter_default_24_hours() {
        let cleaner = TempCleaner::new().unwrap();

        assert!(cleaner.min_age.is_some(), "Default min_age should be set");
        assert_eq!(
            cleaner.min_age.unwrap(),
            Duration::from_secs(24 * 3600),
            "Default min_age should be 24 hours"
        );
    }

    #[test]
    fn test_age_filter_custom() {
        let cleaner = TempCleaner::with_age_filter(Some(48)).unwrap();

        assert!(cleaner.min_age.is_some());
        assert_eq!(
            cleaner.min_age.unwrap(),
            Duration::from_secs(48 * 3600),
            "Custom min_age should be 48 hours"
        );
    }

    #[test]
    fn test_age_filter_none() {
        let cleaner = TempCleaner::with_age_filter(None).unwrap();

        assert!(cleaner.min_age.is_none(), "No age filter should be set");
    }

    #[test]
    fn test_should_delete_file_respects_min_age() {
        use std::fs::File;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("recent_file.txt");

        // Create a fresh file
        File::create(&file_path).unwrap();
        let metadata = std::fs::metadata(&file_path).unwrap();

        // File should NOT be deleted if min_age is 24 hours
        let min_age = Some(Duration::from_secs(24 * 3600));
        assert!(
            !should_delete_file(&metadata, min_age, None),
            "Recent file should not be deleted with 24h min_age"
        );

        // File SHOULD be deleted if min_age is 0 seconds
        let min_age_zero = Some(Duration::from_secs(0));
        assert!(
            should_delete_file(&metadata, min_age_zero, None),
            "File should be deleted with 0s min_age"
        );

        // File SHOULD be deleted if no age filter
        assert!(
            should_delete_file(&metadata, None, None),
            "File should be deleted with no age filter"
        );
    }

    #[test]
    fn test_dry_run_does_not_delete() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // Create test files
        let file1 = test_path.join("test1.txt");
        let file2 = test_path.join("test2.txt");
        std::fs::write(&file1, "content1").unwrap();
        std::fs::write(&file2, "content2").unwrap();

        let mut cleaner = TempCleaner::new().unwrap();
        cleaner.directories = vec![test_path.to_string_lossy().to_string()];
        cleaner.min_age = None; // Delete all files

        // Dry run
        let stats = cleaner.clean(true, |_, _| {}).unwrap();

        // Files should still exist
        assert!(file1.exists(), "File should still exist after dry run");
        assert!(file2.exists(), "File should still exist after dry run");
        assert_eq!(stats.deleted_files, 2, "Stats should show 2 files would be deleted");
    }

    #[test]
    fn test_actual_cleanup_deletes_files() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // Create test files
        let file1 = test_path.join("test1.txt");
        let file2 = test_path.join("test2.txt");
        std::fs::write(&file1, "content1").unwrap();
        std::fs::write(&file2, "content2").unwrap();

        let mut cleaner = TempCleaner::new().unwrap();
        cleaner.directories = vec![test_path.to_string_lossy().to_string()];
        cleaner.min_age = None; // Delete all files

        // Actual cleanup
        let stats = cleaner.clean(false, |_, _| {}).unwrap();

        // Files should be deleted
        assert!(!file1.exists(), "File should be deleted after cleanup");
        assert!(!file2.exists(), "File should be deleted after cleanup");
        assert_eq!(stats.deleted_files, 2, "Stats should show 2 files deleted");
    }

    #[test]
    fn test_cleanup_tracks_failed_files() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // Create a file and a read-only file
        let normal_file = test_path.join("normal.txt");
        let readonly_file = test_path.join("readonly.txt");

        std::fs::write(&normal_file, "content").unwrap();
        std::fs::write(&readonly_file, "content").unwrap();

        // Make file read-only
        #[cfg(windows)]
        {
            let mut perms = std::fs::metadata(&readonly_file).unwrap().permissions();
            perms.set_readonly(true);
            std::fs::set_permissions(&readonly_file, perms).unwrap();
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&readonly_file).unwrap().permissions();
            perms.set_mode(0o444); // Read-only
            std::fs::set_permissions(&readonly_file, perms).unwrap();
        }

        let mut cleaner = TempCleaner::new().unwrap();
        cleaner.directories = vec![test_path.to_string_lossy().to_string()];
        cleaner.min_age = None;

        let stats = cleaner.clean(false, |_, _| {}).unwrap();

        // Should have tried to delete both files
        assert_eq!(stats.total_files, 2, "Should detect 2 files");

        // Normal file should be deleted
        assert!(!normal_file.exists(), "Normal file should be deleted");

        // At least one file should have failed (the readonly one)
        // Note: On some systems, even readonly files can be deleted by owner
        if readonly_file.exists() {
            assert!(stats.failed_files >= 1, "Should track failed file");
        }
    }

    #[test]
    fn test_scan_counts_files_correctly() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // Create files with known sizes
        std::fs::write(test_path.join("file1.txt"), "12345").unwrap(); // 5 bytes
        std::fs::write(test_path.join("file2.txt"), "1234567890").unwrap(); // 10 bytes

        let mut cleaner = TempCleaner::new().unwrap();
        cleaner.directories = vec![test_path.to_string_lossy().to_string()];
        cleaner.min_age = None;

        let stats = cleaner.scan();

        assert_eq!(stats.total_files, 2, "Should count 2 files");
        assert_eq!(stats.total_size, 15, "Should sum to 15 bytes");
    }

    #[test]
    fn test_cleanup_stats_default() {
        let stats = CleanupStats::default();

        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.deleted_files, 0);
        assert_eq!(stats.deleted_size, 0);
        assert_eq!(stats.failed_files, 0);
        assert_eq!(stats.skipped_files, 0);
        assert_eq!(stats.permission_errors, 0);
        assert_eq!(stats.other_errors, 0);
        assert!(!stats.was_cancelled);
        assert!(stats.inaccessible_dirs.is_empty());
    }

    #[test]
    fn test_progress_callback_called() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // Create 3 files
        for i in 0..3 {
            std::fs::write(test_path.join(format!("file{}.txt", i)), "test").unwrap();
        }

        let mut cleaner = TempCleaner::new().unwrap();
        cleaner.directories = vec![test_path.to_string_lossy().to_string()];
        cleaner.min_age = None;

        let mut callback_count = 0;
        let mut last_processed = 0;

        cleaner.clean(true, |processed, total| {
            callback_count += 1;
            last_processed = processed;
            assert!(processed <= total, "Processed should not exceed total");
        }).unwrap();

        assert!(callback_count > 0, "Callback should be called");
        assert_eq!(last_processed, 3, "Should process all 3 files");
    }

    #[test]
    fn test_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path();

        // Create nested structure
        let subdir = test_path.join("subdir");
        std::fs::create_dir(&subdir).unwrap();

        std::fs::write(test_path.join("root.txt"), "root").unwrap();
        std::fs::write(subdir.join("nested.txt"), "nested").unwrap();

        let mut cleaner = TempCleaner::new().unwrap();
        cleaner.directories = vec![test_path.to_string_lossy().to_string()];
        cleaner.min_age = None;

        let stats = cleaner.scan();

        assert_eq!(stats.total_files, 2, "Should find files in nested directories");
    }

    #[test]
    fn test_nonexistent_directory_tracked() {
        let mut cleaner = TempCleaner::new().unwrap();
        cleaner.directories = vec!["/nonexistent/path/12345".to_string()];

        let stats = cleaner.scan();

        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.inaccessible_dirs.len(), 1);
        assert_eq!(stats.inaccessible_dirs[0], "/nonexistent/path/12345");
    }
}
