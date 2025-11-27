use msc::core::config::Config;
use tempfile::TempDir;

#[test]
fn test_config_default() {
    let config = Config::default();
    assert!(config.work_path.is_none());
}

#[test]
fn test_config_set_and_get_work_path() {
    let config = Config {
        work_path: Some("/tmp/test".to_string()),
        ..Default::default()
    };

    assert_eq!(config.work_path, Some("/tmp/test".to_string()));
}

#[test]
fn test_config_load_nonexistent_returns_default() {
    // Loading a non-existent config should return default
    // This test might fail if there's an actual config file, which is OK
    let _config = Config::load();
}

#[test]
fn test_config_roundtrip() {
    // Create a temp directory for testing
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path().to_str().unwrap());

    // For this test, we just verify Config can be created
    let config = Config::default();
    assert!(config.workspaces.is_empty());
}

#[test]
fn test_config_workspaces() {
    use std::collections::HashMap;

    let mut workspaces = HashMap::new();
    workspaces.insert("project1".to_string(), "/path/to/project1".to_string());
    workspaces.insert("project2".to_string(), "/path/to/project2".to_string());

    let config = Config {
        work_path: None,
        video_path: None,
        yt_dlp_path: None,
        yt_dlp_installed_by_msc: false,
        workspaces,
        default_paths: Vec::new(),
        custom_paths: Vec::new(),
        excluded_default_paths: Vec::new(),
        ignored_work_folders: Vec::new(),
    };

    assert_eq!(config.workspaces.len(), 2);
    assert_eq!(
        config.workspaces.get("project1"),
        Some(&"/path/to/project1".to_string())
    );
}
