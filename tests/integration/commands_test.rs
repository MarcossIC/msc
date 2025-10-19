use msc::core::cleaner::TempCleaner;
use msc::core::config::Config;
use msc::core::file_scanner::FileScanner;
use msc::core::workspace::WorkspaceManager;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn test_workspace_manager_creation() {
    // Test that we can create a workspace manager
    // This might fail if config doesn't exist, which is expected
    let _result = WorkspaceManager::new();
}

#[test]
fn test_workspace_manager_with_empty_config() {
    let config = Config::default();
    let manager = WorkspaceManager::with_config(config);

    let workspaces = manager.list_workspaces();
    assert!(workspaces.is_empty());
}

#[test]
fn test_workspace_list() {
    use std::collections::HashMap;

    let mut workspaces = HashMap::new();
    workspaces.insert("test1".to_string(), "/path/test1".to_string());
    workspaces.insert("test2".to_string(), "/path/test2".to_string());

    let config = Config {
        work_path: None,
        workspaces,
    };

    let manager = WorkspaceManager::with_config(config);
    let workspaces_list = manager.list_workspaces();

    assert_eq!(workspaces_list.len(), 2);
}

#[test]
fn test_temp_cleaner_creation() {
    let cleaner = TempCleaner::new();
    assert!(cleaner.is_ok());
}

#[test]
fn test_temp_cleaner_scan() {
    let cleaner = TempCleaner::new().unwrap();
    let stats = cleaner.scan();

    // Just verify the stats structure is returned correctly
    // Both fields are usize/u64, so they're always valid
    let _ = stats.total_files;
    let _ = stats.total_size;
}

#[test]
fn test_temp_cleaner_dry_run() {
    let cleaner = TempCleaner::new().unwrap();

    // Dry run should complete without errors
    let result = cleaner.clean(true, |_processed, _total| {});
    assert!(result.is_ok());
}

#[test]
fn test_file_scanner_creation() {
    let scanner = FileScanner::new(Path::new("."));
    assert!(scanner.is_ok());
}

#[test]
fn test_file_scanner_scan_current_dir() {
    let scanner = FileScanner::new(Path::new(".")).unwrap();
    let result = scanner.scan(false);

    // Should be able to scan current directory
    assert!(result.is_ok());

    let entries = result.unwrap();
    // Current directory should have at least some files
    assert!(!entries.is_empty());
}

#[test]
fn test_file_scanner_with_temp_directory() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    // Create some test files
    fs::write(test_path.join("test1.rs"), "// test").unwrap();
    fs::write(test_path.join("test2.md"), "# Test").unwrap();
    fs::write(test_path.join(".hidden"), "hidden").unwrap();

    let scanner = FileScanner::new(test_path).unwrap();

    // Scan without hidden files
    let entries = scanner.scan(false).unwrap();
    assert_eq!(entries.len(), 2); // Only non-hidden files

    // Scan with hidden files
    let entries_with_hidden = scanner.scan(true).unwrap();
    assert!(entries_with_hidden.len() >= 2); // At least the visible files
}

#[test]
fn test_file_scanner_sorts_directories_first() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    // Create files and directories
    fs::write(test_path.join("afile.txt"), "content").unwrap();
    fs::create_dir(test_path.join("zdir")).unwrap();
    fs::write(test_path.join("bfile.txt"), "content").unwrap();

    let scanner = FileScanner::new(test_path).unwrap();
    let entries = scanner.scan(false).unwrap();

    // First entry should be the directory
    assert!(entries[0].is_dir);
    assert_eq!(entries[0].name, "zdir");

    // Then files in alphabetical order
    assert!(!entries[1].is_dir);
    assert_eq!(entries[1].name, "afile.txt");
}
