# Security Documentation

---

## Overview

This document serves as the main entry point for all security-related documentation for the MSC CLI project. MSC implements multiple layers of defense to protect against command injection, privilege escalation, and other security threats.

---

## Quick Links

### For Developers
- **[Security Architecture](./security-architecture.md)** - Threat model, defense layers, attack surface analysis
- **[Security Implementation Plan](./security-implementation-plan.md)** - Detailed implementation plan for security fixes
- **[Security Audit Findings](./security-audit-findings.md)** - Original vulnerability assessment
- **[Security Audit Review](./security-audit-review.md)** - Review and prioritization of findings

### For Security Researchers
- **[Security Recommendations](./security-recommendations.md)** - Known issues and mitigation strategies
- **[Security Architecture](./security-architecture.md)** - Attack surface and defenses

### For Users
- See [User Security Guidelines](#user-security-guidelines) below

---

## Security Features

### ✅ Implemented Protections

#### 1. Command Injection Prevention
**What:** Validates all alias commands before execution
**How:**
- Unicode normalization (NFKC) to prevent homoglyph attacks
- Character blacklisting for shell metacharacters (`;` `|` `&` `` ` `` etc.)
- Control character filtering (0x00-0x1F, 0x7F)
- Keyword blacklisting (`exec`, `eval`, etc.)

**Example:**
```bash
# ❌ Blocked
msc alias add bad "ls; rm -rf /"

# ❌ Blocked (Unicode attack - Greek Question Mark)
msc alias add trick "ls\u037E rm -rf /"

# ✅ Allowed
msc alias add good "ls -la"
```

#### 2. PowerShell Elevation Security (Windows)
**What:** Safely elevates privileges on Windows without injection vulnerabilities
**How:**
- Uses `-EncodedCommand` with Base64-encoded UTF-16 LE
- Proper argument escaping within PowerShell
- Arguments treated as data, not code

#### 3. Symlink Attack Prevention (Unix)
**What:** Prevents attackers from using symlinks to overwrite sensitive files
**How:**
- Checks if target file is a symlink before writing
- Refuses to overwrite symlinks with security error

**Attack Prevented:**
```bash
# Attacker creates malicious symlink
ln -s ~/.bashrc ~/.config/msc/aliases/bin/evil

# Victim tries to create alias
msc alias add evil "echo safe"
# ✅ BLOCKED: "Security Error: Refusing to overwrite symlink"
```

#### 4. Path Traversal Prevention
**What:** Prevents directory traversal attacks
**How:**
- Rejects absolute paths
- Blocks `..` and `./` in paths
- Null byte protection

#### 5. Memory Safety
**What:** Prevents buffer overflows, use-after-free, data races
**How:** Implemented in Rust (memory-safe language)

---

### ⚠️ Partial Protections

#### Binary Download Verification
**Status:** Framework implemented, hash database pending
**What:** Verifies downloaded binaries (yt-dlp, ffmpeg, wget)
**Current:** Downloads over HTTPS only
**Planned:** SHA256 checksum verification against known-good hashes

---

### ❌ Not Yet Protected

#### Environment Variable Injection (Unix)
**Risk:** Library injection via `LD_PRELOAD`, command shadowing via `PATH`
**Impact:** Medium
**Mitigation:** Manual - don't run untrusted aliases with modified env vars
**Planned:** Auto-sanitization in generated scripts

---

## Security Testing

### Running Security Tests

```bash
# Run all security tests
cargo test --test security_audit_test
cargo test --test security_bypass_tests

# Run specific test
cargo test test_unicode_normalization_bypass

# Run with verbose output
cargo test -- --nocapture
```

### Test Coverage

- **21 security tests** across 2 test suites
- Platform-specific tests for Windows and Unix
- Tests for all major attack surfaces
- Bypass technique validation

---

## User Security Guidelines

### Creating Safe Aliases

✅ **DO:**
- Use simple commands: `ls`, `cat`, `echo`
- Use command flags: `ls -la`, `grep -i`
- Chain with spaces only: `ls -la`

❌ **DON'T:**
- Use command separators: `;` `&&` `||`
- Use pipes: `|`
- Use command substitution: `` `...` `` `$(...)`
- Use redirections: `>` `<`
- Use wildcards in untrusted contexts: `*` `?`

### Reviewing Aliases

```bash
# List all aliases
msc alias list

# View generated script (Unix)
cat ~/.config/msc/aliases/bin/<alias-name>

# View generated script (Windows)
type %USERPROFILE%\.config\msc\aliases\<alias-name>.exe
```

### Removing Suspicious Aliases

```bash
# Remove specific alias
msc alias remove <name>

# Remove all aliases (if compromised)
msc alias clear
```

---

## Reporting Security Vulnerabilities

### How to Report

**DO NOT** create public GitHub issues for security vulnerabilities.

**Instead:**
1. Email: [security contact - TBD]
2. Or: Create a private security advisory on GitHub
3. Include:
   - MSC version (`msc --version`)
   - Operating system
   - Steps to reproduce
   - Expected vs actual behavior
   - Proof of concept (if applicable)

### What to Expect

- **Acknowledgment:** Within 48 hours
- **Initial Assessment:** Within 7 days
- **Fix Timeline:** Depends on severity
  - Critical: 1-7 days
  - High: 7-30 days
  - Medium: 30-90 days

### Hall of Fame

Security researchers who responsibly disclose vulnerabilities will be credited here.

---

## Security Considerations for Developers

### When Adding New Features

1. **Input Validation**
   - Always validate user input
   - Use `validation::validate_alias_command()` for commands
   - Use `validation::validate_directory_path()` for paths
   - Use `validation::validate_url()` for URLs

2. **File Operations**
   - Check for symlinks before writing (Unix)
   - Use `fs::symlink_metadata()` instead of `fs::metadata()`
   - Set appropriate file permissions (0o755 for scripts)

3. **Command Execution**
   - Never use `sh -c` with user input
   - Use `Command::new()` with individual arguments
   - Don't interpolate user input into shell commands

4. **Privilege Escalation**
   - Use `elevation::build_elevation_command()` on Windows
   - Never use `-Command`, always use `-EncodedCommand`

### Code Review Checklist

- [ ] User input validated before use
- [ ] No shell metacharacters in executed commands
- [ ] No string interpolation in shell commands
- [ ] Symlinks checked before file writes (Unix)
- [ ] Paths validated (no traversal)
- [ ] Security tests added for new attack surfaces
- [ ] Documentation updated

---

## Version History

### v0.1.0 (2025-11-28)
- Initial security implementation
- Unicode normalization
- Symlink protection
- PowerShell injection prevention
- Comprehensive security test suite

---

## Additional Resources

### External Resources
- [OWASP Command Injection](https://owasp.org/www-community/attacks/Command_Injection)
- [CWE-78: OS Command Injection](https://cwe.mitre.org/data/definitions/78.html)
- [Unicode Security Considerations](https://unicode.org/reports/tr36/)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)

### Internal Documentation
- [Architecture](./ARCHITECTURE.md) - Overall system architecture
- [Contributing Guidelines](../CONTRIBUTING.md) - How to contribute safely
