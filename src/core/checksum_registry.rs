use std::collections::HashMap;
use once_cell::sync::Lazy;

/// A verified checksum entry with metadata
#[derive(Debug, Clone)]
pub struct VerifiedChecksum {
    /// The SHA256 checksum
    pub hash: String,
    /// Official source where this checksum was verified
    pub source: String,
    /// Date when this checksum was verified (YYYY-MM-DD)
    pub verified_date: String,
}

impl VerifiedChecksum {
    pub fn new(hash: &str, source: &str, verified_date: &str) -> Self {
        Self {
            hash: hash.to_string(),
            source: source.to_string(),
            verified_date: verified_date.to_string(),
        }
    }
}

/// Registry of manually verified checksums as fallback
///
/// # How to Update This Registry
///
/// When adding new checksums, follow these steps:
///
/// 1. **yt-dlp**:
///    - Go to https://github.com/yt-dlp/yt-dlp/releases
///    - Find the release version
///    - Download the `.sha256` file for your platform
///    - Verify the checksum matches the binary
///
/// 2. **ffmpeg**:
///    - Go to https://github.com/BtbN/FFmpeg-Builds/releases
///    - Find the release with tag matching version (e.g., n7.1)
///    - Check SHA256SUMS file for your platform
///    - Verify against the downloaded binary
///
/// 3. **wget**:
///    - Go to https://eternallybored.org/misc/wget/
///    - Or check https://github.com/mirror/wget/releases
///    - Download checksum file
///    - Verify against binary
///
/// # Format
///
/// ```rust
/// use std::collections::HashMap;
/// use msc::core::checksum_registry::VerifiedChecksum;
/// 
/// let mut map = HashMap::new();
/// map.insert(
///     ("tool_name", "version", "platform", "arch"),
///     VerifiedChecksum::new(
///         "sha256_hash_64_chars",
///         "https://source.url/releases",
///         "YYYY-MM-DD"
///     )
/// );
/// ```
pub struct ChecksumRegistry {
    checksums: HashMap<(String, String, String, String), VerifiedChecksum>,
}

impl ChecksumRegistry {
    /// Create a new registry with verified checksums
    pub fn new() -> Self {
        let mut registry = Self {
            checksums: HashMap::new(),
        };

        registry.load_verified_checksums();
        registry
    }

    /// Load verified checksums into the registry
    ///
    /// ⚠️ IMPORTANT: These checksums must be verified manually before adding
    ///
    /// To verify a checksum:
    /// 1. Download the tool from official source
    /// 2. Calculate SHA256: `sha256sum tool.exe` or `certutil -hashfile tool.exe SHA256`
    /// 3. Compare with official checksum from release page
    /// 4. Only add if they match exactly
    fn load_verified_checksums(&mut self) {
        // =================================================================
        // yt-dlp checksums
        // Source: https://github.com/yt-dlp/yt-dlp/releases
        // =================================================================

        // 📝 INSTRUCTIONS FOR ADDING CHECKSUMS:
        //
        // 1. Go to https://github.com/yt-dlp/yt-dlp/releases
        // 2. Find the version you want to add
        // 3. Download both the binary AND the .sha256 file
        // 4. Verify: certutil -hashfile yt-dlp.exe SHA256 (Windows)
        //           or: sha256sum yt-dlp (Linux/Mac)
        // 5. Compare with the .sha256 file content
        // 6. If they match, add below using self.add()
        //
        // Example (UNCOMMENT AND REPLACE WITH REAL HASH):
        // self.add("yt-dlp", "2024.12.13", "windows", "x86_64",
        //     VerifiedChecksum::new(
        //         "YOUR_VERIFIED_SHA256_HASH_HERE_64_CHARACTERS_EXACTLY",
        //         "https://github.com/yt-dlp/yt-dlp/releases/tag/2024.12.13",
        //         "2024-12-13"
        //     )
        // );

        // =================================================================
        // ffmpeg checksums
        // Source: https://github.com/BtbN/FFmpeg-Builds/releases
        // =================================================================

        // 📝 INSTRUCTIONS:
        //
        // 1. Go to https://github.com/BtbN/FFmpeg-Builds/releases
        // 2. Download the release matching your version (e.g., n7.1)
        // 3. Download the SHA256SUMS file
        // 4. Find the line for your platform (e.g., win64-gpl)
        // 5. Verify the downloaded file matches the hash
        // 6. Add below:
        //
        // self.add("ffmpeg", "7.1", "windows", "x86_64",
        //     VerifiedChecksum::new(
        //         "YOUR_VERIFIED_SHA256_HASH_HERE",
        //         "https://github.com/BtbN/FFmpeg-Builds/releases/tag/latest",
        //         "2026-01-12"
        //     )
        // );

        // =================================================================
        // wget checksums
        // Source: https://eternallybored.org/misc/wget/
        // =================================================================

        // 📝 INSTRUCTIONS:
        //
        // 1. Go to https://eternallybored.org/misc/wget/
        // 2. Download the latest wget version for Windows
        // 3. Calculate hash: certutil -hashfile wget.exe SHA256
        // 4. Verify against official source
        // 5. Add below:
        //
        // self.add("wget", "1.21.4", "windows", "x86_64",
        //     VerifiedChecksum::new(
        //         "YOUR_VERIFIED_SHA256_HASH_HERE",
        //         "https://eternallybored.org/misc/wget/",
        //         "2026-01-12"
        //     )
        // );

        // =================================================================
        // TEMPLATE FOR ADDING NEW CHECKSUMS:
        // =================================================================
        //
        // self.add("tool_name", "version", "platform", "arch",
        //     VerifiedChecksum::new(
        //         "64_character_sha256_hash_in_lowercase_hex",
        //         "https://official.source/releases/version",
        //         "YYYY-MM-DD"
        //     )
        // );
        //
        // Platforms: "windows", "linux", "macos"
        // Architectures: "x86_64", "aarch64", "i686"
        //
        // ⚠️ NEVER commit fake or unverified checksums!
        // ⚠️ Always verify against official sources!
    }

    /// Add a checksum to the registry
    ///
    /// This method is public within the crate for testing purposes and for adding
    /// verified checksums in `load_verified_checksums()`.
    pub(crate) fn add(&mut self, tool: &str, version: &str, platform: &str, arch: &str, checksum: VerifiedChecksum) {
        let key = (
            tool.to_string(),
            version.to_string(),
            platform.to_string(),
            arch.to_string(),
        );

        // Validate checksum format
        if checksum.hash.len() != 64 || !checksum.hash.chars().all(|c| c.is_ascii_hexdigit()) {
            log::warn!(
                "Invalid SHA256 checksum format for {} {} ({}/{}): {}",
                tool, version, platform, arch, checksum.hash
            );
            return;
        }

        self.checksums.insert(key, checksum);
    }

    /// Get a verified checksum from the registry
    pub fn get(&self, tool: &str, version: &str, platform: &str, arch: &str) -> Option<&VerifiedChecksum> {
        let key = (
            tool.to_string(),
            version.to_string(),
            platform.to_string(),
            arch.to_string(),
        );

        self.checksums.get(&key)
    }

    /// Get all available versions for a tool and platform
    pub fn get_available_versions(&self, tool: &str, platform: &str, arch: &str) -> Vec<String> {
        self.checksums
            .keys()
            .filter(|(t, _, p, a)| t == tool && p == platform && a == arch)
            .map(|(_, v, _, _)| v.clone())
            .collect()
    }

    /// Check if a checksum exists for the given parameters
    pub fn has_checksum(&self, tool: &str, version: &str, platform: &str, arch: &str) -> bool {
        self.get(tool, version, platform, arch).is_some()
    }

    /// Get the count of verified checksums in the registry
    pub fn count(&self) -> usize {
        self.checksums.len()
    }
}

impl Default for ChecksumRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global registry instance
pub static CHECKSUM_REGISTRY: Lazy<ChecksumRegistry> = Lazy::new(ChecksumRegistry::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = ChecksumRegistry::new();
        // Initially empty as we need to add real checksums
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_add_and_get() {
        let mut registry = ChecksumRegistry::new();

        // Valid 64-character SHA256 hash (all zeros for testing)
        let checksum = VerifiedChecksum::new(
            "0000000000000000000000000000000000000000000000000000000000000000",
            "https://example.com/releases",
            "2024-01-01"
        );

        registry.add("test-tool", "1.0.0", "windows", "x86_64", checksum);

        let retrieved = registry.get("test-tool", "1.0.0", "windows", "x86_64");
        assert!(retrieved.is_some());
        assert_eq!(
            retrieved.unwrap().hash,
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn test_invalid_checksum_format() {
        let mut registry = ChecksumRegistry::new();

        // Too short
        let invalid = VerifiedChecksum::new(
            "abc123",
            "https://example.com",
            "2024-01-01"
        );

        registry.add("test-tool", "1.0.0", "windows", "x86_64", invalid);

        let retrieved = registry.get("test-tool", "1.0.0", "windows", "x86_64");
        assert!(retrieved.is_none(), "Invalid checksum should not be added");
    }

    #[test]
    fn test_get_available_versions() {
        let mut registry = ChecksumRegistry::new();

        // Valid 64-character SHA256 hashes
        registry.add("tool", "1.0", "windows", "x86_64", VerifiedChecksum::new(
            "1111111111111111111111111111111111111111111111111111111111111111",
            "https://example.com", "2024-01-01"
        ));
        registry.add("tool", "2.0", "windows", "x86_64", VerifiedChecksum::new(
            "2222222222222222222222222222222222222222222222222222222222222222",
            "https://example.com", "2024-01-01"
        ));

        let versions = registry.get_available_versions("tool", "windows", "x86_64");
        assert_eq!(versions.len(), 2);
        assert!(versions.contains(&"1.0".to_string()));
        assert!(versions.contains(&"2.0".to_string()));
    }

    #[test]
    fn test_has_checksum() {
        let mut registry = ChecksumRegistry::new();

        // Valid 64-character SHA256 hash
        registry.add("tool", "1.0", "windows", "x86_64", VerifiedChecksum::new(
            "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
            "https://example.com", "2024-01-01"
        ));

        assert!(registry.has_checksum("tool", "1.0", "windows", "x86_64"));
        assert!(!registry.has_checksum("tool", "2.0", "windows", "x86_64"));
    }
}
