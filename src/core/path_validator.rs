use std::path::{Path, PathBuf};
use anyhow::Result;

pub struct PathValidator {
    forbidden_paths: Vec<String>,
    forbidden_patterns: Vec<String>,
}

impl PathValidator {
    pub fn new() -> Self {
        Self {
            forbidden_paths: Self::get_forbidden_paths(),
            forbidden_patterns: Self::get_forbidden_patterns(),
        }
    }

    fn get_forbidden_paths() -> Vec<String> {
        let mut paths = Vec::new();

        #[cfg(windows)]
        {
            // Directorios cr�ticos de Windows
            paths.extend(vec![
                "C:\\Windows".to_string(),
                "C:\\Windows\\System32".to_string(),
                "C:\\Windows\\SysWOW64".to_string(),
                "C:\\Program Files".to_string(),
                "C:\\Program Files (x86)".to_string(),
                "C:\\ProgramData".to_string(),
                "C:\\Users".to_string(), // Ra�z de usuarios
                "C:\\".to_string(),       // Ra�z del sistema
            ]);

            // Directorio de usuario actual (ra�z)
            if let Ok(userprofile) = std::env::var("USERPROFILE") {
                paths.push(userprofile);
            }
        }

        #[cfg(unix)]
        {
            paths.extend(vec![
                "/".to_string(),
                "/bin".to_string(),
                "/sbin".to_string(),
                "/usr".to_string(),
                "/usr/bin".to_string(),
                "/usr/sbin".to_string(),
                "/etc".to_string(),
                "/var".to_string(),
                "/sys".to_string(),
                "/proc".to_string(),
                "/dev".to_string(),
                "/boot".to_string(),
            ]);

            // Home directory ra�z
            if let Ok(home) = std::env::var("HOME") {
                paths.push(home);
            }
        }

        paths
    }

    fn get_forbidden_patterns() -> Vec<String> {
        vec![
            "system32".to_string(),
            "program files".to_string(),
            "programdata".to_string(),
        ]
    }

    /// Validate if a path is safe to use for cleanup
    pub fn validate_path(&self, path: &Path) -> Result<ValidationResult> {
        // 1. Verificar que existe
        if !path.exists() {
            return Ok(ValidationResult::Error(
                "Path does not exist".to_string(),
            ));
        }

        // 2. Verificar que es directorio
        if !path.is_dir() {
            return Ok(ValidationResult::Error(
                "Path is not a directory".to_string(),
            ));
        }

        // 3. Canonicalizar (resolver symlinks)
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                return Ok(ValidationResult::Error(format!(
                    "Cannot resolve path: {}",
                    e
                )))
            }
        };

        // 4. Verificar que no es symlink a ubicaci�n peligrosa
        #[cfg(unix)]
        {
            if path.read_link().is_ok() {
                return Ok(ValidationResult::Warning(
                    format!(
                        "Path is a symbolic link to: {}\nThis could be dangerous. Are you sure?",
                        canonical.display()
                    ),
                    canonical.clone(),
                ));
            }
        }

        // 5. Verificar contra lista de rutas prohibidas
        let canonical_str = canonical.to_string_lossy().to_lowercase();

        for forbidden in &self.forbidden_paths {
            let forbidden_lower = forbidden.to_lowercase();

            // Normalizar separadores de ruta y quitar prefijos de Windows (\\?\)
            let canonical_normalized = canonical_str
                .replace('/', "\\")
                .replace("\\\\?\\", "");
            let forbidden_normalized = forbidden_lower
                .replace('/', "\\");

            // Verificar coincidencia exacta
            if canonical_normalized == forbidden_normalized {
                return Ok(ValidationResult::Forbidden(format!(
                    "Cannot add system directory: {}",
                    canonical.display()
                )));
            }

            // Verificar si está dentro de directorio prohibido
            if canonical_normalized.starts_with(&forbidden_normalized) {
                // Excepción: subdirectorios conocidos como seguros
                if !self.is_safe_subdirectory(&canonical) {
                    return Ok(ValidationResult::Forbidden(format!(
                        "Cannot add directory inside system location: {}\nParent directory {} is protected",
                        canonical.display(),
                        forbidden
                    )));
                }
            }
        }

        // 6. Verificar patrones peligrosos en la ruta
        for pattern in &self.forbidden_patterns {
            if canonical_str.contains(pattern) {
                // Verificar si es realmente un directorio del sistema
                if !self.is_safe_subdirectory(&canonical) {
                    return Ok(ValidationResult::Warning(
                        format!(
                            "Path contains potentially dangerous pattern '{}': {}\nPlease verify this is correct.",
                            pattern,
                            canonical.display()
                        ),
                        canonical.clone(),
                    ));
                }
            }
        }

        // 7. Verificar que no est� en uso por el sistema
        if self.is_system_active_directory(&canonical) {
            return Ok(ValidationResult::Warning(
                format!(
                    "Directory appears to be actively used by system: {}\nCleaning this may cause issues.",
                    canonical.display()
                ),
                canonical.clone(),
            ));
        }

        Ok(ValidationResult::Safe(canonical))
    }

    fn is_safe_subdirectory(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        // Subdirectorios conocidos como seguros para limpieza
        let safe_subdirs = vec![
            "\\appdata\\local\\temp",
            "\\temp",
            "/tmp",
            "\\cache",
            "/cache",
        ];

        for safe_subdir in safe_subdirs {
            if path_str.contains(safe_subdir) {
                return true;
            }
        }

        false
    }

    fn is_system_active_directory(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        // Directorios conocidos como activos del sistema
        let active_dirs = vec![
            "\\windows\\winsxs",
            "\\windows\\servicing",
            "\\windows\\logs",
            "/var/log",
            "/var/run",
        ];

        for active_dir in active_dirs {
            if path_str.contains(active_dir) {
                return true;
            }
        }

        false
    }
}

impl Default for PathValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum ValidationResult {
    Safe(PathBuf),
    Warning(String, PathBuf),  // (mensaje de advertencia, ruta canónica)
    Forbidden(String),
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forbidden_system_directories() {
        let validator = PathValidator::new();

        #[cfg(windows)]
        {
            let system32 = Path::new("C:\\Windows\\System32");
            if system32.exists() {
                let result = validator.validate_path(system32).unwrap();
                assert!(matches!(result, ValidationResult::Forbidden(_)));
            }
        }

        #[cfg(unix)]
        {
            let usr_bin = Path::new("/usr/bin");
            if usr_bin.exists() {
                let result = validator.validate_path(usr_bin).unwrap();
                assert!(matches!(result, ValidationResult::Forbidden(_)));
            }
        }
    }

    #[test]
    fn test_nonexistent_path() {
        let validator = PathValidator::new();
        let result = validator
            .validate_path(Path::new("/nonexistent/path/that/does/not/exist"))
            .unwrap();
        assert!(matches!(result, ValidationResult::Error(_)));
    }

    #[test]
    fn test_safe_temp_directory() {
        let validator = PathValidator::new();

        #[cfg(windows)]
        {
            if let Ok(temp) = std::env::var("TEMP") {
                let temp_path = Path::new(&temp);
                if temp_path.exists() {
                    let result = validator.validate_path(temp_path).unwrap();
                    assert!(matches!(result, ValidationResult::Safe(_)));
                }
            }
        }

        #[cfg(unix)]
        {
            let tmp = Path::new("/tmp");
            if tmp.exists() {
                let result = validator.validate_path(tmp).unwrap();
                assert!(matches!(result, ValidationResult::Safe(_)));
            }
        }
    }

    #[test]
    fn test_file_instead_of_directory() {
        use tempfile::NamedTempFile;

        let validator = PathValidator::new();
        let temp_file = NamedTempFile::new().unwrap();

        let result = validator.validate_path(temp_file.path()).unwrap();
        assert!(
            matches!(result, ValidationResult::Error(_)),
            "Should reject file paths"
        );
    }

    #[test]
    fn test_root_directory_forbidden() {
        let validator = PathValidator::new();

        #[cfg(windows)]
        {
            let root = Path::new("C:\\");
            if root.exists() {
                let result = validator.validate_path(root).unwrap();
                assert!(
                    matches!(result, ValidationResult::Forbidden(_)),
                    "Root directory C:\\ should be forbidden"
                );
            }
        }

        #[cfg(unix)]
        {
            let root = Path::new("/");
            if root.exists() {
                let result = validator.validate_path(root).unwrap();
                assert!(
                    matches!(result, ValidationResult::Forbidden(_)),
                    "Root directory / should be forbidden"
                );
            }
        }
    }

    #[test]
    fn test_program_files_forbidden() {
        let validator = PathValidator::new();

        #[cfg(windows)]
        {
            let program_files = Path::new("C:\\Program Files");
            if program_files.exists() {
                let result = validator.validate_path(program_files).unwrap();
                assert!(
                    matches!(result, ValidationResult::Forbidden(_)),
                    "Program Files should be forbidden"
                );
            }
        }
    }

    #[test]
    fn test_user_profile_root_forbidden() {
        let validator = PathValidator::new();

        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let profile_path = Path::new(&userprofile);
            if profile_path.exists() {
                let result = validator.validate_path(profile_path).unwrap();
                assert!(
                    matches!(result, ValidationResult::Forbidden(_)),
                    "User profile root should be forbidden"
                );
            }
        }

        #[cfg(unix)]
        {
            if let Ok(home) = std::env::var("HOME") {
                let home_path = Path::new(&home);
                if home_path.exists() {
                    let result = validator.validate_path(home_path).unwrap();
                    assert!(
                        matches!(result, ValidationResult::Forbidden(_)),
                        "Home directory should be forbidden"
                    );
                }
            }
        }
    }

    #[test]
    fn test_safe_subdirectory_logic() {
        use tempfile::TempDir;

        let validator = PathValidator::new();

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Should be safe since it's in temp
        if temp_path.exists() {
            let result = validator.validate_path(temp_path).unwrap();
            assert!(
                matches!(result, ValidationResult::Safe(_)),
                "Temp directory should be safe"
            );
        }
    }

    #[test]
    fn test_validator_default() {
        let validator = PathValidator::default();
        assert!(!validator.forbidden_paths.is_empty(), "Default validator should have forbidden paths");
        assert!(!validator.forbidden_patterns.is_empty(), "Default validator should have forbidden patterns");
    }

    #[test]
    fn test_is_safe_subdirectory() {
        let validator = PathValidator::new();

        #[cfg(windows)]
        {
            // Test safe subdirectories
            let safe_paths = vec![
                Path::new("C:\\Users\\TestUser\\AppData\\Local\\Temp"),
                Path::new("C:\\Temp"),
            ];

            for path in safe_paths {
                assert!(
                    validator.is_safe_subdirectory(path),
                    "Path {:?} should be recognized as safe subdirectory",
                    path
                );
            }

            // Test unsafe subdirectories
            let unsafe_paths = vec![
                Path::new("C:\\Windows\\System32"),
                Path::new("C:\\Program Files\\MyApp"),
            ];

            for path in unsafe_paths {
                assert!(
                    !validator.is_safe_subdirectory(path),
                    "Path {:?} should NOT be recognized as safe subdirectory",
                    path
                );
            }
        }

        #[cfg(unix)]
        {
            // Test safe subdirectories
            let safe_paths = vec![
                Path::new("/tmp"),
                Path::new("/var/cache"),
            ];

            for path in safe_paths {
                assert!(
                    validator.is_safe_subdirectory(path),
                    "Path {:?} should be recognized as safe subdirectory",
                    path
                );
            }

            // Test unsafe subdirectories
            let unsafe_paths = vec![
                Path::new("/usr/bin"),
                Path::new("/etc"),
            ];

            for path in unsafe_paths {
                assert!(
                    !validator.is_safe_subdirectory(path),
                    "Path {:?} should NOT be recognized as safe subdirectory",
                    path
                );
            }
        }
    }

    #[test]
    fn test_is_system_active_directory() {
        let validator = PathValidator::new();

        #[cfg(windows)]
        {
            let active_dirs = vec![
                Path::new("C:\\Windows\\WinSxS"),
                Path::new("C:\\Windows\\Servicing"),
                Path::new("C:\\Windows\\Logs"),
            ];

            for path in active_dirs {
                assert!(
                    validator.is_system_active_directory(path),
                    "Path {:?} should be recognized as active system directory",
                    path
                );
            }
        }

        #[cfg(unix)]
        {
            let active_dirs = vec![
                Path::new("/var/log"),
                Path::new("/var/run"),
            ];

            for path in active_dirs {
                assert!(
                    validator.is_system_active_directory(path),
                    "Path {:?} should be recognized as active system directory",
                    path
                );
            }
        }
    }

    #[test]
    fn test_dangerous_patterns_detection() {
        let validator = PathValidator::new();

        // The patterns are checked case-insensitively and in the path string
        assert!(
            validator.forbidden_patterns.contains(&"system32".to_string()),
            "Should include system32 pattern"
        );
        assert!(
            validator.forbidden_patterns.contains(&"program files".to_string()),
            "Should include program files pattern"
        );
    }

    #[test]
    fn test_multiple_validation_calls() {
        use tempfile::TempDir;

        let validator = PathValidator::new();
        let temp_dir = TempDir::new().unwrap();

        // Multiple calls should return consistent results
        for _ in 0..3 {
            let result = validator.validate_path(temp_dir.path()).unwrap();
            assert!(
                matches!(result, ValidationResult::Safe(_)),
                "Should consistently validate safe paths"
            );
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_specific_forbidden_paths() {
        let validator = PathValidator::new();

        let forbidden = vec![
            "C:\\Windows",
            "C:\\Windows\\System32",
            "C:\\Windows\\SysWOW64",
            "C:\\Program Files",
            "C:\\Program Files (x86)",
            "C:\\ProgramData",
        ];

        for path_str in forbidden {
            assert!(
                validator.forbidden_paths.iter().any(|p| p.to_lowercase() == path_str.to_lowercase()),
                "Should include {} in forbidden paths",
                path_str
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_unix_specific_forbidden_paths() {
        let validator = PathValidator::new();

        let forbidden = vec![
            "/bin",
            "/sbin",
            "/usr",
            "/usr/bin",
            "/usr/sbin",
            "/etc",
            "/var",
            "/sys",
            "/proc",
            "/dev",
            "/boot",
        ];

        for path_str in forbidden {
            assert!(
                validator.forbidden_paths.contains(&path_str.to_string()),
                "Should include {} in forbidden paths",
                path_str
            );
        }
    }
}
