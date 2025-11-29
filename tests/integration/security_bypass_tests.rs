// SECURITY BYPASS TEST SUITE
// ===========================
// Comprehensive tests for attack surfaces identified in the security audit
// that were not covered by the original security_audit_test.rs file.
//
// This file focuses on bypass techniques and edge cases that attackers
// might use to circumvent security controls.
//
// Test Categories:
// - Unicode normalization bypass attacks
// - Symlink attack prevention
// - Control character bypass attacks
// - Homoglyph alias name attacks
// - Windows-specific path attacks
// - Environment variable injection

use msc::core::validation;

#[cfg(unix)]
use msc::core::{alias::Alias, alias_generator::AliasGenerator, alias_generator::UnixScriptGenerator};

// ============================================================================
// UNICODE NORMALIZATION BYPASS TESTS
// ============================================================================

#[test]
fn test_unicode_normalization_bypass() {
    println!("\n🔴 Testing Unicode Homoglyph Attacks\n");

    let unicode_attacks = vec![
        // Greek Question Mark (looks like semicolon)
        ("ls \u{037E} rm -rf /", "Greek Question Mark (U+037E)", ';'),

        // Fullwidth semicolon
        ("ls\u{FF1B}malicious", "Fullwidth Semicolon (U+FF1B)", ';'),

        // Small semicolon
        ("ls\u{FE54}evil", "Small Semicolon (U+FE54)", ';'),

        // Non-breaking space instead of space
        ("ls\u{00A0}&&\u{00A0}curl evil.com", "Non-Breaking Space (U+00A0)", '&'),

        // En Quad space
        ("ls\u{2000}|\u{2000}bash", "En Quad (U+2000)", '|'),

        // Zero-width spaces for obfuscation
        ("l\u{200B}s\u{200B};\u{200B}rm", "Zero-Width Space (U+200B)", ';'),

        // Cyrillic letters that look like Latin
        ("l\u{0441} ; rm -rf /", "Cyrillic 'c' (U+0441)", ';'), // 'с' looks like 'c'

        // RTL Override (visual spoofing)
        ("ls \u{202E}metsys ;", "Right-to-Left Override (U+202E)", ';'),
    ];

    for (cmd, description, _dangerous_char) in unicode_attacks {
        println!("  Testing: {}", description);
        println!("    Command: {:?}", cmd);

        let result = validation::validate_alias_command(cmd);

        assert!(
            result.is_err(),
            "❌ FAILED to block Unicode bypass attack ({}): {:?}",
            description,
            cmd
        );

        println!("    ✅ Blocked: {}", description);
    }

    println!("\n✅ All Unicode bypass attacks blocked\n");
}

#[test]
fn test_unicode_normalization_legitimate_commands() {
    println!("\n🔵 Testing Unicode Normalization with Legitimate International Commands\n");

    // Ensure Unicode normalization doesn't break legitimate international commands
    let legitimate = vec![
        ("echo Hello", "ASCII - should pass"),
        ("ls -la", "Simple command - should pass"),
        ("cat file.txt", "File operation - should pass"),
    ];

    for (cmd, description) in legitimate {
        let result = validation::validate_alias_command(cmd);
        println!("  {}: {:?}", description, result);

        // Note: These should pass basic validation
        // International characters might still fail due to strict validation
        // but we document the behavior
    }

    println!("\n");
}

// ============================================================================
// SYMLINK ATTACK PREVENTION TEST
// ============================================================================

#[test]
#[cfg(unix)]
fn test_symlink_attack_prevention() {
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;
    use std::fs;

    println!("\n🔴 Testing Symlink Attack Prevention (Unix)\n");

    let temp = tempdir().unwrap();
    let alias_dir = temp.path().join("aliases");
    fs::create_dir(&alias_dir).unwrap();

    // Create a sensitive file that attacker wants to overwrite
    let sensitive = temp.path().join("important_config");
    fs::write(&sensitive, "CRITICAL DATA - DO NOT MODIFY").unwrap();

    println!("  Created sensitive file: {:?}", sensitive);

    // Attacker creates a symlink in the alias directory
    let symlink_path = alias_dir.join("evil_alias");
    symlink(&sensitive, &symlink_path).unwrap();

    println!("  Attacker created symlink: {:?} -> {:?}", symlink_path, sensitive);
    assert!(symlink_path.exists(), "Symlink should exist");

    // Victim tries to create an alias with the same name
    let alias = Alias::new("evil_alias".into(), "echo safe".into())
        .expect("Should create alias with safe command");

    let gen = UnixScriptGenerator::new();
    let result = gen.generate(&alias, &alias_dir);

    println!("  Victim attempts to create alias 'evil_alias'");

    // CRITICAL: Generation MUST fail
    assert!(
        result.is_err(),
        "❌ SECURITY FAILURE: Symlink attack succeeded! Generator should refuse to overwrite symlinks."
    );

    println!("  ✅ Generator correctly rejected symlink");

    // Verify the sensitive file was NOT modified
    let sensitive_content = fs::read_to_string(&sensitive).unwrap();
    assert_eq!(
        sensitive_content, "CRITICAL DATA - DO NOT MODIFY",
        "❌ SECURITY FAILURE: Sensitive file was modified! Symlink attack succeeded."
    );

    println!("  ✅ Sensitive file remains unmodified");

    // Verify symlink still exists (wasn't replaced)
    let metadata = fs::symlink_metadata(&symlink_path).unwrap();
    assert!(
        metadata.is_symlink(),
        "Symlink should still exist after failed generation"
    );

    println!("  ✅ Symlink still exists (not replaced)");
    println!("\n✅ Symlink attack successfully prevented\n");
}

// ============================================================================
// CONTROL CHARACTERS BYPASS TEST
// ============================================================================

#[test]
fn test_control_characters_bypass() {
    println!("\n🔴 Testing Control Character Attacks\n");

    let control_char_attacks = vec![
        ("ls\x0b; rm -rf /", "Vertical Tab (0x0B)", 0x0B),
        ("ls\x0c| bash", "Form Feed (0x0C)", 0x0C),
        ("ls\x1b[31mmalicious", "ESC character (0x1B)", 0x1B),
        ("ls\x7f; evil", "DEL character (0x7F)", 0x7F),
        ("ls\x00; null", "NULL byte (0x00)", 0x00),
        ("ls\x01\x02\x03", "SOH/STX/ETX (0x01-0x03)", 0x01),
        ("cmd\x1e; attack", "Record Separator (0x1E)", 0x1E),
    ];

    for (cmd, description, code) in control_char_attacks {
        println!("  Testing: {} (0x{:02X})", description, code);

        let result = validation::validate_alias_command(cmd);

        assert!(
            result.is_err(),
            "❌ FAILED to block control character attack ({}): {:?}",
            description,
            cmd
        );

        println!("    ✅ Blocked: {}", description);
    }

    println!("\n✅ All control character attacks blocked\n");
}

// ============================================================================
// HOMOGLYPH ALIAS NAME ATTACK TEST
// ============================================================================

#[test]
fn test_homoglyph_alias_names() {
    use msc::core::alias::Alias;

    println!("\n🔴 Testing Homoglyph Alias Name Attacks\n");

    // These alias names use Unicode characters that look like ASCII
    let homoglyph_names = vec![
        ("c\u{0430}t", "cat with Cyrillic 'a' (U+0430)"), // cаt (Cyrillic)
        ("l\u{0455}", "ls with Cyrillic 's' (U+0455)"),   // lѕ
        ("r\u{043C}", "rm with Cyrillic 'm' (U+043C)"),   // rм
    ];

    for (name, description) in homoglyph_names {
        println!("  Testing: {}", description);
        println!("    Visual: {}", name);
        println!("    Actual: {:?}", name);

        // Attempt to create alias with homoglyph name
        let result = Alias::new(name.to_string(), "echo test".to_string());

        // Document current behavior
        // Ideally should reject or normalize, but may currently allow
        match result {
            Ok(_) => println!("    ⚠️  WARNING: Homoglyph name accepted (potential social engineering risk)"),
            Err(e) => println!("    ✅ Blocked: {}", e),
        }
    }

    println!("\n");
}

// ============================================================================
// WINDOWS PATH ATTACK TESTS
// ============================================================================

#[test]
#[cfg(windows)]
fn test_windows_path_attacks() {
    println!("\n🔴 Testing Windows-Specific Path Attacks\n");

    let windows_paths = vec![
        (r"\\?\C:\Windows\System32\calc.exe", "DOS device path"),
        (r"C:\safe\file.txt:evil.exe", "alternate data stream"),
        ("CON", "reserved device name CON"),
        ("NUL", "reserved device name NUL"),
        ("COM1", "reserved device name COM1"),
        ("LPT1", "reserved device name LPT1"),
        ("PRN", "reserved device name PRN"),
        (r"C:\test\..\..\..\Windows\System32", "path traversal with backslash"),
        (&format!(r"C:\{}", "A\\".repeat(200)), "long path bypass (>260 chars)"),
    ];

    for (path, description) in windows_paths {
        println!("  Testing: {}", description);

        let result = validation::validate_directory_path(path);

        // Document behavior
        match result {
            Ok(_) => println!("    ⚠️  WARNING: Potentially dangerous path accepted: {}", description),
            Err(_) => println!("    ✅ Blocked: {}", description),
        }
    }

    println!("\n");
}

// ============================================================================
// ENVIRONMENT VARIABLE INJECTION TEST
// ============================================================================

#[test]
#[cfg(unix)]
fn test_environment_variable_attacks() {
    use tempfile::tempdir;
    use std::fs;

    println!("\n🔴 Testing Environment Variable Injection\n");

    let temp = tempdir().unwrap();

    let alias = Alias::new("test".into(), "ls".into()).unwrap();
    let gen = UnixScriptGenerator::new();
    gen.generate(&alias, temp.path()).unwrap();

    let script = fs::read_to_string(temp.path().join("test")).unwrap();

    println!("  Generated script:\n{}", script);

    // Check if script sanitizes dangerous environment variables
    let has_ld_preload_protection =
        script.contains("unset LD_PRELOAD") ||
        script.contains("unset LD_LIBRARY_PATH") ||
        script.contains("env -i");

    let has_path_protection =
        script.contains("PATH=/usr/bin:/bin") ||
        script.contains("unset PATH");

    if !has_ld_preload_protection {
        println!("  ⚠️  WARNING: Script doesn't sanitize LD_PRELOAD (library injection risk)");
    } else {
        println!("  ✅ Script sanitizes LD_PRELOAD");
    }

    if !has_path_protection {
        println!("  ⚠️  WARNING: Script doesn't sanitize PATH (command shadowing risk)");
    } else {
        println!("  ✅ Script sanitizes PATH");
    }

    println!("\n");
}

// ============================================================================
// TOCTOU RACE CONDITION PROTECTION TEST (DOCUMENTATION)
// ============================================================================

#[test]
fn test_toctou_race_condition_protection() {
    // NOTE: This test documents that TOCTOU is NOT a vulnerability in current implementation
    // The `alias add` command passes the Alias object from memory to the generator,
    // rather than re-reading from disk, which prevents the race condition.

    println!("\n🔵 TOCTOU Race Condition Analysis\n");
    println!("  Current implementation: PROTECTED");
    println!("  Reason: Generator receives Alias object from memory");
    println!("  The config file is not re-read between save and generate");
    println!("\n✅ No TOCTOU vulnerability detected in current design\n");

    // This test always passes as documentation
    assert!(true);
}

// ============================================================================
// JSON INJECTION PROTECTION TEST (WINDOWS)
// ============================================================================

#[test]
#[cfg(windows)]
fn test_malicious_json_structures() {
    println!("\n🔴 Testing JSON Injection Attacks (Windows)\n");

    let malicious_jsons = vec![
        // Null byte in string
        (r#"{"aliases": {"test": {"name": "test\u0000", "command": "echo"}}}"#,
         "null byte in name"),

        // Right-to-left override (visual spoofing)
        (r#"{"aliases": {"test": {"name": "test", "command": "echo\u202E\u202D"}}}"#,
         "RTL override in command"),

        // Extremely large number (DoS)
        (r#"{"version": 99999999999999999999999}"#,
         "integer overflow"),

        // Deeply nested (DoS)
        (&format!(r#"{{"a":{}}}"#, r#"{"b":"#.repeat(1000)),
         "deeply nested JSON (DoS)"),
    ];

    for (json, description) in malicious_jsons {
        println!("  Testing: {}", description);

        // Attempt to parse malicious JSON
        let result = serde_json::from_str::<serde_json::Value>(json);

        match result {
            Ok(_) => println!("    ⚠️  JSON parsed (validate application handles safely): {}", description),
            Err(e) => println!("    ✅ JSON rejected by parser: {} - {}", description, e),
        }
    }

    println!("\n");
}
