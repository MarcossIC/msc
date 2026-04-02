use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Cache entry for a checksum with expiration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecksumCacheEntry {
    /// The SHA256 checksum
    pub hash: String,
    /// When this entry was fetched
    pub fetched_at: DateTime<Utc>,
    /// Source URL where this checksum was obtained
    pub source: String,
}

/// Persistent cache for checksums with TTL
#[derive(Debug, Serialize, Deserialize)]
pub struct ChecksumCache {
    /// Map of (tool, version, platform, arch) -> ChecksumCacheEntry
    entries: HashMap<String, ChecksumCacheEntry>,
    /// Cache file path
    #[serde(skip)]
    cache_path: PathBuf,
}

impl ChecksumCache {
    /// Cache validity duration (7 days)
    const CACHE_TTL_DAYS: i64 = 7;

    /// Create a new cache instance
    pub fn new(cache_path: PathBuf) -> Self {
        Self {
            entries: HashMap::new(),
            cache_path,
        }
    }

    /// Load cache from disk
    pub fn load(cache_path: PathBuf) -> Result<Self> {
        if !cache_path.exists() {
            return Ok(Self::new(cache_path));
        }

        let data = fs::read_to_string(&cache_path).context("Failed to read checksum cache file")?;

        let mut cache: ChecksumCache =
            serde_json::from_str(&data).context("Failed to parse checksum cache")?;

        cache.cache_path = cache_path;

        Ok(cache)
    }

    /// Save cache to disk
    pub fn save(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent).context("Failed to create cache directory")?;
        }

        let json = serde_json::to_string_pretty(&self).context("Failed to serialize cache")?;

        fs::write(&self.cache_path, json).context("Failed to write cache file")?;

        Ok(())
    }

    /// Build cache key from tool metadata
    fn build_key(tool: &str, version: &str, platform: &str, arch: &str) -> String {
        format!("{}:{}:{}:{}", tool, version, platform, arch)
    }

    /// Get a checksum from cache if it exists and is not expired
    pub fn get(&self, tool: &str, version: &str, platform: &str, arch: &str) -> Option<String> {
        let key = Self::build_key(tool, version, platform, arch);

        if let Some(entry) = self.entries.get(&key) {
            // Check if entry is still valid
            let age = Utc::now().signed_duration_since(entry.fetched_at);
            if age.num_days() < Self::CACHE_TTL_DAYS {
                log::debug!(
                    "Cache hit for {} {} ({}/{}), age: {} days",
                    tool,
                    version,
                    platform,
                    arch,
                    age.num_days()
                );
                return Some(entry.hash.clone());
            } else {
                log::debug!(
                    "Cache expired for {} {} ({}/{}), age: {} days",
                    tool,
                    version,
                    platform,
                    arch,
                    age.num_days()
                );
            }
        }

        None
    }

    /// Store a checksum in cache
    pub fn set(
        &mut self,
        tool: &str,
        version: &str,
        platform: &str,
        arch: &str,
        hash: String,
        source: String,
    ) {
        let key = Self::build_key(tool, version, platform, arch);

        let entry = ChecksumCacheEntry {
            hash,
            fetched_at: Utc::now(),
            source,
        };

        self.entries.insert(key, entry);

        log::debug!(
            "Cached checksum for {} {} ({}/{})",
            tool,
            version,
            platform,
            arch
        );
    }

    /// Remove expired entries
    pub fn cleanup_expired(&mut self) -> usize {
        let initial_count = self.entries.len();

        self.entries.retain(|_, entry| {
            let age = Utc::now().signed_duration_since(entry.fetched_at);
            age.num_days() < Self::CACHE_TTL_DAYS
        });

        let removed = initial_count - self.entries.len();
        if removed > 0 {
            log::info!("Cleaned up {} expired cache entries", removed);
        }

        removed
    }

    /// Clear all cache entries
    pub fn clear(&mut self) {
        let count = self.entries.len();
        self.entries.clear();
        log::info!("Cleared {} cache entries", count);
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let now = Utc::now();
        let mut valid = 0;
        let mut expired = 0;

        for entry in self.entries.values() {
            let age = now.signed_duration_since(entry.fetched_at);
            if age.num_days() < Self::CACHE_TTL_DAYS {
                valid += 1;
            } else {
                expired += 1;
            }
        }

        CacheStats {
            total_entries: self.entries.len(),
            valid_entries: valid,
            expired_entries: expired,
        }
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    pub total_entries: usize,
    pub valid_entries: usize,
    pub expired_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cache_basic_operations() {
        let temp = tempdir().unwrap();
        let cache_path = temp.path().join("checksums.json");

        let mut cache = ChecksumCache::new(cache_path.clone());

        // Set a value
        cache.set(
            "yt-dlp",
            "2024.11.18",
            "windows",
            "x86_64",
            "abc123".to_string(),
            "https://github.com/yt-dlp/yt-dlp/releases".to_string(),
        );

        // Get the value
        let hash = cache.get("yt-dlp", "2024.11.18", "windows", "x86_64");
        assert_eq!(hash, Some("abc123".to_string()));

        // Non-existent value
        let hash = cache.get("ffmpeg", "7.0", "windows", "x86_64");
        assert_eq!(hash, None);
    }

    #[test]
    fn test_cache_persistence() {
        let temp = tempdir().unwrap();
        let cache_path = temp.path().join("checksums.json");

        // Create and save cache
        {
            let mut cache = ChecksumCache::new(cache_path.clone());
            cache.set(
                "yt-dlp",
                "2024.11.18",
                "windows",
                "x86_64",
                "abc123".to_string(),
                "https://github.com".to_string(),
            );
            cache.save().unwrap();
        }

        // Load cache
        {
            let cache = ChecksumCache::load(cache_path).unwrap();
            let hash = cache.get("yt-dlp", "2024.11.18", "windows", "x86_64");
            assert_eq!(hash, Some("abc123".to_string()));
        }
    }

    #[test]
    fn test_cache_stats() {
        let temp = tempdir().unwrap();
        let cache_path = temp.path().join("checksums.json");
        let mut cache = ChecksumCache::new(cache_path);

        cache.set(
            "tool1",
            "1.0",
            "windows",
            "x86_64",
            "hash1".to_string(),
            "src1".to_string(),
        );
        cache.set(
            "tool2",
            "2.0",
            "linux",
            "x86_64",
            "hash2".to_string(),
            "src2".to_string(),
        );

        let stats = cache.stats();
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.valid_entries, 2);
        assert_eq!(stats.expired_entries, 0);
    }
}
