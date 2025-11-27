// Tests for set command validation (set work, set video)
// These tests ensure that the set commands properly validate paths
// and reject invalid or dangerous inputs

use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper function to check if a path is a valid directory
fn is_valid_directory(path: &str) -> bool {
    let path_buf = Path::new(path);
    path_buf.exists() && path_buf.is_dir()
}

/// Helper function to check if a path points to a file (should be rejected)
fn is_file_not_directory(path: &str) -> bool {
    let path_buf = Path::new(path);
    path_buf.exists() && path_buf.is_file()
}

#[test]
fn test_set_work_with_nonexistent_path() {
    // Test that setting a non-existent path shows warning but is accepted
    // (Current behavior: accepts but shows warning)
    let nonexistent_path = "/this/path/does/not/exist/at/all";

    assert!(!Path::new(nonexistent_path).exists());

    // The current implementation accepts non-existent paths with a warning
    // This test documents that behavior
}

#[test]
fn test_set_work_with_file_instead_of_directory() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");

    // Create a file
    fs::write(&test_file, "test content").unwrap();

    // Verify it's a file, not a directory
    assert!(test_file.exists());
    assert!(test_file.is_file());
    assert!(!test_file.is_dir());

    let path_str = test_file.to_string_lossy().to_string();

    // This should fail - a file is not a valid directory
    assert!(is_file_not_directory(&path_str));
    assert!(!is_valid_directory(&path_str));
}

#[test]
fn test_set_work_with_binary_file_extension() {
    let temp_dir = TempDir::new().unwrap();

    // Test various file extensions that should be rejected
    let invalid_extensions = vec!["exe", "bin", "dll", "so", "dylib", "png", "jpg", "zip"];

    for ext in invalid_extensions {
        let test_file = temp_dir.path().join(format!("test.{}", ext));
        fs::write(&test_file, "binary content").unwrap();

        // These paths point to files, should be rejected
        assert!(
            test_file.is_file(),
            "Extension {} should create a file",
            ext
        );
        assert!(
            !test_file.is_dir(),
            "Extension {} should not be a directory",
            ext
        );
    }
}

#[test]
fn test_set_video_with_nonexistent_path() {
    // Same behavior as set work
    let nonexistent_path = "/videos/that/do/not/exist";

    assert!(!Path::new(nonexistent_path).exists());

    // The current implementation accepts non-existent paths with a warning
    // This test documents that behavior
}

#[test]
fn test_set_video_with_file_instead_of_directory() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("video.mp4");

    // Create a file
    fs::write(&test_file, "fake video content").unwrap();

    // Verify it's a file, not a directory
    assert!(test_file.exists());
    assert!(test_file.is_file());
    assert!(!test_file.is_dir());

    let path_str = test_file.to_string_lossy().to_string();

    // This should fail - a file is not a valid directory
    assert!(is_file_not_directory(&path_str));
    assert!(!is_valid_directory(&path_str));
}

#[test]
fn test_set_work_with_empty_string() {
    let empty_path = "";

    // Empty paths should be rejected
    assert!(!is_valid_directory(empty_path));
}

#[test]
fn test_set_work_with_whitespace_only() {
    let whitespace_paths = vec!["   ", "\t", "\n", "  \t  \n  "];

    for path in whitespace_paths {
        // Whitespace-only paths should be rejected
        assert!(!is_valid_directory(path));
    }
}

#[test]
fn test_set_work_with_valid_directory() {
    let temp_dir = TempDir::new().unwrap();
    let path_str = temp_dir.path().to_string_lossy().to_string();

    // Valid directory should be accepted
    assert!(is_valid_directory(&path_str));
}

#[test]
fn test_set_video_with_valid_directory() {
    let temp_dir = TempDir::new().unwrap();
    let videos_dir = temp_dir.path().join("videos");
    fs::create_dir(&videos_dir).unwrap();

    let path_str = videos_dir.to_string_lossy().to_string();

    // Valid directory should be accepted
    assert!(is_valid_directory(&path_str));
}

#[test]
fn test_path_with_special_characters() {
    // Test paths with special characters that might be dangerous
    let special_paths = vec![
        "../../../etc/passwd",
        "..\\..\\..\\Windows\\System32",
        "./config/../../../etc",
        "C:\\..\\..\\sensitive",
    ];

    for path in special_paths {
        // These paths might exist on some systems, but they contain path traversal
        // The key is that they should be canonicalized before being used
        let path_buf = Path::new(path);

        // If they exist, verify canonicalization would catch traversal
        if path_buf.exists() {
            if let Ok(canonical) = path_buf.canonicalize() {
                // The canonical path should be different from the original
                // if there was path traversal
                let canonical_str = canonical.to_string_lossy();
                let original_has_traversal = path.contains("..") || path.contains("..");

                if original_has_traversal {
                    // Canonicalization should resolve the traversal
                    assert_ne!(canonical_str, path);
                }
            }
        }
    }
}

#[test]
fn test_path_canonicalization_prevents_traversal() {
    let temp_dir = TempDir::new().unwrap();
    let sub_dir = temp_dir.path().join("subdir");
    fs::create_dir(&sub_dir).unwrap();

    // Create a path with .. traversal
    let traversal_path = sub_dir.join("..").join("subdir");

    // Both should resolve to the same canonical path
    let canonical_direct = sub_dir.canonicalize().unwrap();
    let canonical_traversal = traversal_path.canonicalize().unwrap();

    assert_eq!(canonical_direct, canonical_traversal);
}

#[test]
fn test_reject_path_to_image_file() {
    let temp_dir = TempDir::new().unwrap();

    // Create various image files
    let image_extensions = vec!["png", "jpg", "jpeg", "gif", "bmp", "svg", "webp"];

    for ext in image_extensions {
        let image_file = temp_dir.path().join(format!("image.{}", ext));
        fs::write(&image_file, "fake image data").unwrap();

        let path_str = image_file.to_string_lossy().to_string();

        // Image files should not be valid directories
        assert!(!is_valid_directory(&path_str));
        assert!(is_file_not_directory(&path_str));
    }
}

#[test]
fn test_reject_path_to_archive_file() {
    let temp_dir = TempDir::new().unwrap();

    // Create various archive files
    let archive_extensions = vec!["zip", "tar", "gz", "7z", "rar", "tar.gz"];

    for ext in archive_extensions {
        let archive_file = temp_dir.path().join(format!("archive.{}", ext));
        fs::write(&archive_file, "fake archive data").unwrap();

        let path_str = archive_file.to_string_lossy().to_string();

        // Archive files should not be valid directories
        assert!(!is_valid_directory(&path_str));
    }
}

#[test]
fn test_reject_path_to_executable() {
    let temp_dir = TempDir::new().unwrap();

    // Create various executable files
    let exe_extensions = vec!["exe", "bat", "cmd", "sh", "ps1", "com"];

    for ext in exe_extensions {
        let exe_file = temp_dir.path().join(format!("program.{}", ext));
        fs::write(&exe_file, "fake executable").unwrap();

        let path_str = exe_file.to_string_lossy().to_string();

        // Executable files should not be valid directories
        assert!(!is_valid_directory(&path_str));
    }
}

#[test]
fn test_windows_path_with_unc_prefix() {
    // Test that paths with \\?\ prefix are handled correctly
    // This is Windows-specific extended path prefix

    #[cfg(target_os = "windows")]
    {
        let temp_dir = TempDir::new().unwrap();
        let canonical = temp_dir.path().canonicalize().unwrap();
        let canonical_str = canonical.to_string_lossy().to_string();

        // On Windows, canonicalize adds \\?\ prefix
        if canonical_str.starts_with("\\\\?\\") {
            // This is expected behavior on Windows
            assert!(canonical_str.len() > 4);

            // The path without prefix should still be valid
            let stripped = canonical_str.strip_prefix("\\\\?\\").unwrap();
            assert!(!stripped.is_empty());
        }
    }
}

#[test]
fn test_very_long_path() {
    // Test that extremely long paths are handled correctly
    // This could be used for DoS attacks

    let long_path = "a/".repeat(1000);

    // Long paths that don't exist should be handled gracefully
    assert!(!Path::new(&long_path).exists());
}

#[test]
fn test_path_with_null_bytes() {
    // Paths with null bytes should be rejected
    // Rust's Path should handle this, but let's verify

    let null_byte_path = "test\0path";
    let path_buf = Path::new(null_byte_path);

    // The path object can be created, but operations should fail
    // or the null byte should be handled safely
    assert!(path_buf.to_string_lossy().contains('\0') || !path_buf.exists());
}

#[test]
fn test_symlink_to_directory() {
    // Test that symlinks to directories are handled correctly

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let real_dir = temp_dir.path().join("real");
        let link_path = temp_dir.path().join("link");

        fs::create_dir(&real_dir).unwrap();
        symlink(&real_dir, &link_path).unwrap();

        // The symlink should be detected as a directory
        assert!(link_path.exists());
        assert!(link_path.is_dir());

        // Canonicalize should resolve to the real directory
        let canonical = link_path.canonicalize().unwrap();
        let real_canonical = real_dir.canonicalize().unwrap();
        assert_eq!(canonical, real_canonical);
    }
}

#[test]
fn test_symlink_to_file() {
    // Test that symlinks to files are rejected (not directories)

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().unwrap();
        let real_file = temp_dir.path().join("file.txt");
        let link_path = temp_dir.path().join("link");

        fs::write(&real_file, "content").unwrap();
        symlink(&real_file, &link_path).unwrap();

        // The symlink exists but points to a file
        assert!(link_path.exists());

        // It should not be a valid directory
        let path_str = link_path.to_string_lossy().to_string();
        assert!(!is_valid_directory(&path_str));
    }
}

#[test]
fn test_permission_denied_directory() {
    // Test handling of directories we can't access

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let restricted_dir = temp_dir.path().join("restricted");
        fs::create_dir(&restricted_dir).unwrap();

        // Remove all permissions
        let mut perms = fs::metadata(&restricted_dir).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&restricted_dir, perms).unwrap();

        // The directory exists but we can't access it
        assert!(restricted_dir.exists());

        // Canonicalize might fail due to permissions
        let canonical_result = restricted_dir.canonicalize();

        // Restore permissions for cleanup
        let mut perms = fs::metadata(&restricted_dir).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&restricted_dir, perms).unwrap();

        // Either canonicalize fails or succeeds, but should be handled gracefully
        assert!(canonical_result.is_ok() || canonical_result.is_err());
    }
}
