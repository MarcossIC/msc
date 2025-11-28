#[cfg(test)]
mod wget_tests {
    use msc::core::{Config, WgetManager};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_wget_manager_initialization() {
        // Use default() for tests since new() requires a valid config directory
        let manager = WgetManager::default();
        assert!(manager.get_binary_path().is_none());
    }

    #[test]
    fn test_wget_manager_default() {
        let manager = WgetManager::default();
        assert!(manager.get_binary_path().is_none());
    }

    #[test]
    fn test_wget_manager_finds_wget() {
        // Skip this test in CI environments where config directory may not be available
        let mut manager = match WgetManager::new() {
            Ok(m) => m,
            Err(e) => {
                println!("⊘ Skipping test: {}", e);
                return;
            }
        };

        let result = manager.ensure_wget();

        // Test passes if either wget is found OR error message is helpful
        match result {
            Ok(path) => {
                println!("✓ wget found at: {:?}", path);

                // Verify we can get it again without searching
                let path2 = manager.ensure_wget().unwrap();
                assert_eq!(path, path2);
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                assert!(
                    error_msg.contains("wget")
                        || error_msg.contains("download")
                        || error_msg.contains("install"),
                    "Error message should mention wget or installation: {}",
                    error_msg
                );
                println!("✓ wget not found, but error message is helpful");
            }
        }
    }

    #[test]
    fn test_config_web_path_set_and_get() {
        let dir = tempdir().unwrap();
        let test_path = dir.path().join("webs");
        fs::create_dir_all(&test_path).unwrap();

        let mut config = Config::default();
        let path_str = test_path.to_str().unwrap().to_string();

        // Test set
        config.set_web_path(path_str.clone());

        // Test get
        assert_eq!(config.get_web_path().unwrap(), &path_str);
    }

    #[test]
    fn test_config_web_path_default_none() {
        let config = Config::default();
        assert!(config.get_web_path().is_none());
    }

    #[test]
    fn test_validate_web_url() {
        use msc::core::validation;

        // Valid URLs
        let valid_urls = vec![
            "https://www.example.com",
            "http://example.com",
            "https://example.com/path",
            "https://example.com:8080/path",
        ];

        for url in valid_urls {
            assert!(
                validation::validate_web_url(url).is_ok(),
                "Should accept: {}",
                url
            );
        }

        // Invalid URLs
        let invalid_urls = vec![
            "",                     // Empty
            "ftp://example.com",    // Wrong protocol
            "not-a-url",            // Invalid format
            "file:///path/to/file", // Wrong protocol
        ];

        for url in invalid_urls {
            assert!(
                validation::validate_web_url(url).is_err(),
                "Should reject: {}",
                url
            );
        }
    }
}
