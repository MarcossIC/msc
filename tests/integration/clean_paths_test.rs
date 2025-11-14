use msc::core::config::Config;
use std::env;
use tempfile::TempDir;

#[test]
fn test_clean_paths_exclusion_and_reset() {
    // Create a temp directory for testing
    let temp_dir = TempDir::new().unwrap();
    env::set_var("HOME", temp_dir.path().to_str().unwrap());

    // Create a config with some default paths
    let mut config = Config::default();
    config.sync_default_paths(); // Load system defaults

    let initial_count = config.get_clean_paths().len();
    assert!(initial_count > 0, "Should have default paths");

    // Add a custom path
    let custom_path = "/custom/test/path".to_string();
    assert!(config.add_clean_path(custom_path.clone()));

    // Verify it was added
    let paths_with_custom = config.get_clean_paths();
    assert_eq!(paths_with_custom.len(), initial_count + 1);
    assert!(paths_with_custom.contains(&custom_path));

    // Remove a default path (simulate user removing it)
    let default_path = config.get_default_paths()[0].clone();
    assert!(config.remove_clean_path(&default_path));

    // Verify the default path is excluded
    let paths_after_removal = config.get_clean_paths();
    assert!(!paths_after_removal.contains(&default_path));
    assert_eq!(paths_after_removal.len(), initial_count); // +1 custom, -1 excluded default

    // Verify exclusion was recorded
    assert!(config.excluded_default_paths.contains(&default_path));

    // Now test reset functionality
    config.reset_to_defaults();

    // After reset:
    // 1. Custom paths should be cleared
    assert!(config.get_custom_paths().is_empty());

    // 2. Excluded default paths should be cleared
    assert!(config.excluded_default_paths.is_empty());

    // 3. All default paths should be active again
    let paths_after_reset = config.get_clean_paths();
    assert_eq!(paths_after_reset.len(), initial_count);
    assert!(paths_after_reset.contains(&default_path)); // Restored!
    assert!(!paths_after_reset.contains(&custom_path)); // Removed!
}

#[test]
fn test_is_at_defaults_detection() {
    let temp_dir = TempDir::new().unwrap();
    env::set_var("HOME", temp_dir.path().to_str().unwrap());

    let mut config = Config::default();
    config.sync_default_paths();

    // Initially should be at defaults
    let is_at_defaults =
        config.get_custom_paths().is_empty() && config.excluded_default_paths.is_empty();
    assert!(is_at_defaults);

    // Add custom path - no longer at defaults
    config.add_clean_path("/custom/path".to_string());
    let is_at_defaults =
        config.get_custom_paths().is_empty() && config.excluded_default_paths.is_empty();
    assert!(!is_at_defaults);

    // Reset - back to defaults
    config.reset_to_defaults();
    let is_at_defaults =
        config.get_custom_paths().is_empty() && config.excluded_default_paths.is_empty();
    assert!(is_at_defaults);

    // Exclude a default path - no longer at defaults
    if !config.get_default_paths().is_empty() {
        let default_path = config.get_default_paths()[0].clone();
        config.remove_clean_path(&default_path);
        let is_at_defaults =
            config.get_custom_paths().is_empty() && config.excluded_default_paths.is_empty();
        assert!(!is_at_defaults); // This was the bug!
    }
}

#[test]
fn test_dynamic_sync_preserves_exclusions() {
    let temp_dir = TempDir::new().unwrap();
    env::set_var("HOME", temp_dir.path().to_str().unwrap());

    let mut config = Config::default();
    config.sync_default_paths();

    // Remove a default path
    if !config.get_default_paths().is_empty() {
        let default_path = config.get_default_paths()[0].clone();
        config.remove_clean_path(&default_path);

        let paths_before_sync = config.get_clean_paths();
        assert!(!paths_before_sync.contains(&default_path));

        // Sync again (simulating reload)
        config.sync_default_paths();

        // The excluded path should still be excluded
        let paths_after_sync = config.get_clean_paths();
        assert!(!paths_after_sync.contains(&default_path));
        assert_eq!(paths_before_sync.len(), paths_after_sync.len());
    }
}

#[test]
fn test_remove_custom_vs_default_paths() {
    let temp_dir = TempDir::new().unwrap();
    env::set_var("HOME", temp_dir.path().to_str().unwrap());

    let mut config = Config::default();
    config.sync_default_paths();

    // Add a custom path
    let custom_path = "/custom/test".to_string();
    config.add_clean_path(custom_path.clone());

    // Remove the custom path
    assert!(config.remove_clean_path(&custom_path));

    // Should be removed from custom_paths, NOT added to excluded_default_paths
    assert!(!config.get_custom_paths().contains(&custom_path));
    assert!(!config.excluded_default_paths.contains(&custom_path));
    assert!(!config.get_clean_paths().contains(&custom_path));

    // Now remove a default path if available
    if !config.get_default_paths().is_empty() {
        let default_path = config.get_default_paths()[0].clone();
        assert!(config.remove_clean_path(&default_path));

        // Should be added to excluded_default_paths, not in custom_paths
        assert!(config.excluded_default_paths.contains(&default_path));
        assert!(!config.get_custom_paths().contains(&default_path));
        assert!(!config.get_clean_paths().contains(&default_path));
    }
}
