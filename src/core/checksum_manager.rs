use anyhow::{anyhow, ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use crate::core::checksum_cache::ChecksumCache;
use crate::core::checksum_fetcher::ChecksumFetcher;
use crate::core::checksum_registry::{ChecksumRegistry, CHECKSUM_REGISTRY};

/// Strategy for obtaining checksums
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumStrategy {
    /// Try cache first, then fetch, then fallback to registry
    Auto,
    /// Only use cache (fail if not in cache)
    CacheOnly,
    /// Only fetch from remote (skip cache and registry)
    FetchOnly,
    /// Only use hardcoded registry
    RegistryOnly,
    /// Skip verification entirely (DANGEROUS - only for testing)
    Skip,
}

/// Result of a checksum lookup
#[derive(Debug)]
pub struct ChecksumResult {
    /// The SHA256 hash
    pub hash: String,
    /// Where the checksum came from
    pub source: ChecksumSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChecksumSource {
    Cache,
    Fetched(String), // URL
    Registry(String), // Source URL from registry
    Provided, // Manually provided by user
}

/// Manages SHA256 checksums for downloaded binaries with multi-tier verification
///
/// # Architecture
///
/// The ChecksumManager uses a three-tier system for obtaining checksums:
///
/// 1. **Cache** (checksum_cache.rs): Persistent cache of previously fetched checksums
///    - 7-day TTL
///    - Stored in JSON format
///
/// 2. **Fetcher** (checksum_fetcher.rs): Dynamic fetching from official sources
///    - yt-dlp: GitHub releases API
///    - ffmpeg: BtbN/FFmpeg-Builds releases
///    - wget: GitHub mirror releases
///
/// 3. **Registry** (checksum_registry.rs): Hardcoded verified checksums as fallback
///    - Manually verified checksums
///    - Updated periodically
///
/// # Security
///
/// All checksums are validated to be:
/// - Exactly 64 hexadecimal characters (SHA256 format)
/// - Obtained from official sources
/// - Verified against downloaded binaries
///
/// # Usage
///
/// ```no_run
/// use msc::core::ChecksumManager;
/// use std::path::Path;
///
/// # fn main() -> anyhow::Result<()> {
/// let mut manager = ChecksumManager::new()?;
///
/// // Get checksum with auto strategy (cache -> fetch -> registry)
/// let result = manager.get_checksum("yt-dlp", "2024.11.18")?;
///
/// // Verify a file
/// let path = Path::new("yt-dlp.exe");
/// ChecksumManager::verify_file(&path, &result.hash)?;
/// # Ok(())
/// # }
/// ```
pub struct ChecksumManager {
    cache: ChecksumCache,
    fetcher: ChecksumFetcher,
    strategy: ChecksumStrategy,
}

impl ChecksumManager {
    /// Create a new ChecksumManager with default configuration
    pub fn new() -> Result<Self> {
        let cache_path = Self::get_cache_path()?;
        Self::with_cache_path(cache_path)
    }

    /// Create a ChecksumManager with a custom cache path
    pub fn with_cache_path(cache_path: PathBuf) -> Result<Self> {
        let cache = ChecksumCache::load(cache_path)
            .context("Failed to load checksum cache")?;

        let fetcher = ChecksumFetcher::new()
            .context("Failed to create checksum fetcher")?;

        Ok(Self {
            cache,
            fetcher,
            strategy: ChecksumStrategy::Auto,
        })
    }

    /// Set the checksum strategy
    pub fn with_strategy(mut self, strategy: ChecksumStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Get the default cache path
    fn get_cache_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow!("Could not determine cache directory"))?;

        let msc_cache = cache_dir.join("msc");
        Ok(msc_cache.join("checksums.json"))
    }

    /// Get checksum for a tool with current platform detection
    pub fn get_checksum(&mut self, tool: &str, version: &str) -> Result<ChecksumResult> {
        let (platform, arch) = Self::detect_platform();
        self.get_checksum_for_platform(tool, version, &platform, &arch)
    }

    /// Get checksum for a specific platform and architecture
    pub fn get_checksum_for_platform(
        &mut self,
        tool: &str,
        version: &str,
        platform: &str,
        arch: &str,
    ) -> Result<ChecksumResult> {
        log::info!(
            "Getting checksum for {} {} ({}/{})",
            tool, version, platform, arch
        );

        match self.strategy {
            ChecksumStrategy::Skip => {
                log::warn!("Checksum verification skipped (DANGEROUS)");
                return Ok(ChecksumResult {
                    hash: "0".repeat(64),
                    source: ChecksumSource::Provided,
                });
            }
            ChecksumStrategy::CacheOnly => {
                return self.get_from_cache(tool, version, platform, arch);
            }
            ChecksumStrategy::FetchOnly => {
                return self.fetch_and_cache(tool, version, platform, arch);
            }
            ChecksumStrategy::RegistryOnly => {
                return self.get_from_registry(tool, version, platform, arch);
            }
            ChecksumStrategy::Auto => {
                // Try cache first
                if let Ok(result) = self.get_from_cache(tool, version, platform, arch) {
                    return Ok(result);
                }

                // Try fetching
                if let Ok(result) = self.fetch_and_cache(tool, version, platform, arch) {
                    return Ok(result);
                }

                // Fallback to registry
                log::info!(
                    "Fetch failed, falling back to registry for {} {}",
                    tool, version
                );
                return self.get_from_registry(tool, version, platform, arch);
            }
        }
    }

    /// Get checksum from cache
    fn get_from_cache(
        &self,
        tool: &str,
        version: &str,
        platform: &str,
        arch: &str,
    ) -> Result<ChecksumResult> {
        self.cache
            .get(tool, version, platform, arch)
            .map(|hash| ChecksumResult {
                hash,
                source: ChecksumSource::Cache,
            })
            .ok_or_else(|| anyhow!("Checksum not found in cache"))
    }

    /// Fetch checksum and store in cache
    fn fetch_and_cache(
        &mut self,
        tool: &str,
        version: &str,
        platform: &str,
        arch: &str,
    ) -> Result<ChecksumResult> {
        let (hash, source) = self.fetcher.fetch_checksum(tool, version, platform, arch)
            .context("Failed to fetch checksum from remote")?;

        // Store in cache
        self.cache.set(tool, version, platform, arch, hash.clone(), source.clone());

        // Save cache to disk
        if let Err(e) = self.cache.save() {
            log::warn!("Failed to save cache: {}", e);
        }

        Ok(ChecksumResult {
            hash,
            source: ChecksumSource::Fetched(source),
        })
    }

    /// Get checksum from hardcoded registry
    fn get_from_registry(
        &self,
        tool: &str,
        version: &str,
        platform: &str,
        arch: &str,
    ) -> Result<ChecksumResult> {
        CHECKSUM_REGISTRY
            .get(tool, version, platform, arch)
            .map(|verified| ChecksumResult {
                hash: verified.hash.clone(),
                source: ChecksumSource::Registry(verified.source.clone()),
            })
            .ok_or_else(|| {
                anyhow!(
                    "No checksum found for {} {} ({}/{}) in registry.\n\
                     Available versions: {:?}",
                    tool,
                    version,
                    platform,
                    arch,
                    CHECKSUM_REGISTRY.get_available_versions(tool, platform, arch)
                )
            })
    }

    /// Detect current platform and architecture
    pub fn detect_platform() -> (String, String) {
        let platform = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "unknown"
        };

        let arch = if cfg!(target_arch = "x86_64") {
            "x86_64"
        } else if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else if cfg!(target_arch = "x86") {
            "i686"
        } else {
            "unknown"
        };

        (platform.to_string(), arch.to_string())
    }

    /// Verify file matches expected SHA256 hash
    pub fn verify_file(path: &Path, expected_hash: &str) -> Result<()> {
        log::info!("Verifying file: {}", path.display());

        let mut file = File::open(path)
            .context(format!("Failed to open file for verification: {}", path.display()))?;

        let mut hasher = Sha256::new();
        io::copy(&mut file, &mut hasher)
            .context("Failed to read file for hashing")?;

        let hash = format!("{:x}", hasher.finalize());

        ensure!(
            hash.eq_ignore_ascii_case(expected_hash),
            "Checksum verification failed!\n\
             File: {}\n\
             Expected: {}\n\
             Got:      {}\n\n\
             This could indicate:\n\
             - File corruption during download\n\
             - Network issues\n\
             - Security compromise (MITM attack)\n\n\
             DO NOT use this file. Please try downloading again.",
            path.display(),
            expected_hash,
            hash
        );

        log::info!("Checksum verification successful");
        Ok(())
    }

    /// Calculate SHA256 hash of a file
    pub fn calculate_hash(path: &Path) -> Result<String> {
        let mut file = File::open(path)
            .context(format!("Failed to open file: {}", path.display()))?;

        let mut hasher = Sha256::new();
        io::copy(&mut file, &mut hasher)
            .context("Failed to read file")?;

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Clean up expired cache entries
    pub fn cleanup_cache(&mut self) -> usize {
        let removed = self.cache.cleanup_expired();

        if removed > 0 {
            if let Err(e) = self.cache.save() {
                log::warn!("Failed to save cache after cleanup: {}", e);
            }
        }

        removed
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> String {
        let stats = self.cache.stats();
        format!(
            "Total: {}, Valid: {}, Expired: {}",
            stats.total_entries, stats.valid_entries, stats.expired_entries
        )
    }

    /// Save the cache to disk
    pub fn save_cache(&self) -> Result<()> {
        self.cache.save()
    }

    // Legacy compatibility methods

    /// Get expected hash (legacy method - redirects to new system)
    #[deprecated(note = "Use get_checksum() instead")]
    pub fn get_expected_hash(tool: &str, version: &str) -> Option<String> {
        let (platform, arch) = Self::detect_platform();

        CHECKSUM_REGISTRY
            .get(tool, version, &platform, &arch)
            .map(|v| v.hash.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_verify_file_correct_hash() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("test.bin");

        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"test content").unwrap();
        drop(file);

        // SHA256 of "test content"
        let expected = "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72";

        let result = ChecksumManager::verify_file(&file_path, expected);
        assert!(result.is_ok(), "Should verify correct hash");
    }

    #[test]
    fn test_verify_file_incorrect_hash() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("test.bin");

        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"test content").unwrap();
        drop(file);

        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let result = ChecksumManager::verify_file(&file_path, wrong_hash);
        assert!(result.is_err(), "Should reject incorrect hash");
    }

    #[test]
    fn test_calculate_hash() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("test.bin");

        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"test content").unwrap();
        drop(file);

        let hash = ChecksumManager::calculate_hash(&file_path).unwrap();

        // SHA256 of "test content"
        let expected = "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72";

        assert_eq!(hash, expected, "Hash should match expected value");
    }

    #[test]
    fn test_manager_creation() {
        let temp = tempdir().unwrap();
        let cache_path = temp.path().join("checksums.json");

        let manager = ChecksumManager::with_cache_path(cache_path);
        assert!(manager.is_ok(), "Should create manager successfully");
    }

    #[test]
    fn test_detect_platform() {
        let (platform, arch) = ChecksumManager::detect_platform();

        assert!(
            platform == "windows" || platform == "linux" || platform == "macos",
            "Platform should be recognized"
        );

        assert!(
            arch == "x86_64" || arch == "aarch64" || arch == "i686",
            "Architecture should be recognized"
        );
    }

    #[test]
    fn test_skip_strategy() {
        let temp = tempdir().unwrap();
        let cache_path = temp.path().join("checksums.json");

        let mut manager = ChecksumManager::with_cache_path(cache_path)
            .unwrap()
            .with_strategy(ChecksumStrategy::Skip);

        let result = manager.get_checksum("any-tool", "any-version");
        assert!(result.is_ok(), "Skip strategy should always succeed");

        let checksum_result = result.unwrap();
        assert_eq!(checksum_result.hash.len(), 64);
        assert_eq!(checksum_result.source, ChecksumSource::Provided);
    }

    #[test]
    fn test_cache_stats() {
        let temp = tempdir().unwrap();
        let cache_path = temp.path().join("checksums.json");

        let manager = ChecksumManager::with_cache_path(cache_path).unwrap();
        let stats = manager.cache_stats();

        assert!(stats.contains("Total:"));
        assert!(stats.contains("Valid:"));
        assert!(stats.contains("Expired:"));
    }

    #[test]
    fn test_registry_only_strategy() {
        let temp = tempdir().unwrap();
        let cache_path = temp.path().join("checksums.json");

        let mut manager = ChecksumManager::with_cache_path(cache_path)
            .unwrap()
            .with_strategy(ChecksumStrategy::RegistryOnly);

        // This will fail since registry is empty by default
        let result = manager.get_checksum("yt-dlp", "2024.11.18");
        assert!(result.is_err(), "Should fail when no registry entry exists");
    }

    // Integration tests (require network, commented out by default)
    /*
    #[test]
    #[ignore]
    fn test_fetch_strategy() {
        let temp = tempdir().unwrap();
        let cache_path = temp.path().join("checksums.json");

        let mut manager = ChecksumManager::with_cache_path(cache_path)
            .unwrap()
            .with_strategy(ChecksumStrategy::FetchOnly);

        let result = manager.get_checksum("yt-dlp", "latest");

        match result {
            Ok(checksum_result) => {
                println!("Fetched hash: {}", checksum_result.hash);
                println!("Source: {:?}", checksum_result.source);
                assert_eq!(checksum_result.hash.len(), 64);
            }
            Err(e) => {
                println!("Warning: Fetch failed (may be rate limited): {}", e);
            }
        }
    }
    */
}
