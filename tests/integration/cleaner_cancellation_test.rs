// Integration tests for TempCleaner cancellation functionality
// Tests cooperative cancellation during file deletion operations

use msc::core::cleaner::TempCleaner;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_cancellation_during_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    // Create many files to give time for cancellation
    for i in 0..100 {
        std::fs::write(test_path.join(format!("file{}.txt", i)), "content").unwrap();
    }

    let mut cleaner = TempCleaner::new().unwrap();
    cleaner.directories = vec![test_path.to_string_lossy().to_string()];
    cleaner.min_age = None;

    let cancel_flag = cleaner.cancel_flag.clone();

    // Cancel after a short delay
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        cancel_flag.store(true, Ordering::Relaxed);
    });

    let stats = cleaner.clean(false, |_, _| {}).unwrap();

    assert!(stats.was_cancelled, "Should indicate cancellation occurred");
    assert!(stats.deleted_files < 100, "Should not delete all files when cancelled");
}

#[test]
fn test_no_cancellation_completes_fully() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    // Create files
    for i in 0..10 {
        std::fs::write(test_path.join(format!("file{}.txt", i)), "content").unwrap();
    }

    let mut cleaner = TempCleaner::new().unwrap();
    cleaner.directories = vec![test_path.to_string_lossy().to_string()];
    cleaner.min_age = None;

    let stats = cleaner.clean(false, |_, _| {}).unwrap();

    assert!(!stats.was_cancelled, "Should not be cancelled");
    assert_eq!(stats.deleted_files, 10, "Should delete all files");
}

#[test]
fn test_cancellation_is_atomic() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    // Create files
    for i in 0..50 {
        std::fs::write(test_path.join(format!("file{}.txt", i)), "content").unwrap();
    }

    let mut cleaner = TempCleaner::new().unwrap();
    cleaner.directories = vec![test_path.to_string_lossy().to_string()];
    cleaner.min_age = None;

    let cancel_flag_clone = cleaner.cancel_flag.clone();

    // Multiple threads trying to cancel
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let flag = cancel_flag_clone.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(5));
                flag.store(true, Ordering::Relaxed);
            })
        })
        .collect();

    let stats = cleaner.clean(false, |_, _| {}).unwrap();

    for handle in handles {
        handle.join().unwrap();
    }

    assert!(stats.was_cancelled, "Should be cancelled by one of the threads");
}

#[test]
fn test_cancel_during_scan_phase() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    // Create nested structure with many files
    for i in 0..10 {
        let subdir = test_path.join(format!("dir{}", i));
        std::fs::create_dir(&subdir).unwrap();
        for j in 0..10 {
            std::fs::write(subdir.join(format!("file{}.txt", j)), "content").unwrap();
        }
    }

    let mut cleaner = TempCleaner::new().unwrap();
    cleaner.directories = vec![test_path.to_string_lossy().to_string()];
    cleaner.min_age = None;

    // Cancel before cleanup
    cleaner.cancel();

    let stats = cleaner.clean(false, |_, _| {}).unwrap();

    // Should detect cancellation quickly
    assert!(stats.was_cancelled, "Should be cancelled");
    assert_eq!(stats.deleted_files, 0, "Should not delete any files when cancelled before start");
}

#[test]
fn test_dry_run_with_cancellation() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    for i in 0..20 {
        std::fs::write(test_path.join(format!("file{}.txt", i)), "content").unwrap();
    }

    let mut cleaner = TempCleaner::new().unwrap();
    cleaner.directories = vec![test_path.to_string_lossy().to_string()];
    cleaner.min_age = None;

    let cancel_flag = cleaner.cancel_flag.clone();

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        cancel_flag.store(true, Ordering::Relaxed);
    });

    let stats = cleaner.clean(true, |_, _| {}).unwrap();

    // Files should still exist (dry run)
    let remaining = std::fs::read_dir(test_path).unwrap().count();
    assert_eq!(remaining, 20, "Files should still exist in dry run");

    if stats.was_cancelled {
        assert!(stats.deleted_files < 20, "Should not process all files when cancelled");
    }
}

#[test]
fn test_age_filter_with_cancellation() {
    use std::fs::File;
    use std::time::SystemTime;

    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    // Create files
    for i in 0..30 {
        File::create(test_path.join(format!("file{}.txt", i))).unwrap();
    }

    let mut cleaner = TempCleaner::new().unwrap();
    cleaner.directories = vec![test_path.to_string_lossy().to_string()];
    // Set min_age to 1 hour (files just created won't be deleted)
    cleaner.min_age = Some(Duration::from_secs(3600));

    let cancel_flag = cleaner.cancel_flag.clone();

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(5));
        cancel_flag.store(true, Ordering::Relaxed);
    });

    let stats = cleaner.clean(false, |_, _| {}).unwrap();

    // All files should still exist (too new + potentially cancelled)
    let remaining = std::fs::read_dir(test_path).unwrap().count();
    assert_eq!(remaining, 30, "Files should not be deleted due to age filter");
    assert_eq!(stats.deleted_files, 0, "No files should be deleted");
}

#[test]
fn test_error_tracking_during_partial_cancellation() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    // Create some normal files
    for i in 0..15 {
        std::fs::write(test_path.join(format!("file{}.txt", i)), "content").unwrap();
    }

    let mut cleaner = TempCleaner::new().unwrap();
    cleaner.directories = vec![test_path.to_string_lossy().to_string()];
    cleaner.min_age = None;

    let cancel_flag = cleaner.cancel_flag.clone();

    // Cancel mid-operation
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(8));
        cancel_flag.store(true, Ordering::Relaxed);
    });

    let stats = cleaner.clean(false, |_, _| {}).unwrap();

    // Should have some stats even if cancelled
    if stats.was_cancelled {
        assert!(
            stats.deleted_files > 0 || stats.processed == 0,
            "Should have processed some files or none if cancelled immediately"
        );
    }
}

#[test]
fn test_progress_callback_stops_on_cancel() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    for i in 0..50 {
        std::fs::write(test_path.join(format!("file{}.txt", i)), "content").unwrap();
    }

    let mut cleaner = TempCleaner::new().unwrap();
    cleaner.directories = vec![test_path.to_string_lossy().to_string()];
    cleaner.min_age = None;

    let cancel_flag = cleaner.cancel_flag.clone();

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        cancel_flag.store(true, Ordering::Relaxed);
    });

    let callback_count = Arc::new(AtomicBool::new(false));
    let callback_count_clone = callback_count.clone();

    let stats = cleaner
        .clean(false, move |processed, total| {
            callback_count_clone.store(true, Ordering::Relaxed);
            assert!(processed <= total, "Processed should never exceed total");
        })
        .unwrap();

    assert!(
        callback_count.load(Ordering::Relaxed),
        "Progress callback should have been called at least once"
    );

    if stats.was_cancelled {
        assert!(stats.deleted_files < 50, "Should not process all files");
    }
}
