use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;

/// GitHub Release Asset information
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// GitHub Release information
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    #[allow(dead_code)] // Present for serde deserialization of GitHub API response
    tag_name: String,
    assets: Vec<GitHubAsset>,
    body: Option<String>,
}

/// Configuration for fetching checksums from different sources
#[derive(Debug, Clone)]
pub struct FetchConfig {
    /// Timeout for HTTP requests
    pub timeout: Duration,
    /// User agent for requests
    pub user_agent: String,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            user_agent: format!("msc/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Fetches checksums from official sources
pub struct ChecksumFetcher {
    client: Client,
    #[allow(dead_code)] // Kept for future extensions (retries, dynamic timeouts, etc.)
    config: FetchConfig,
}

impl ChecksumFetcher {
    /// Create a new fetcher with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(FetchConfig::default())
    }

    /// Create a new fetcher with custom configuration
    pub fn with_config(config: FetchConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .user_agent(&config.user_agent)
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client, config })
    }

    /// Fetch checksum for a specific tool, version, platform and architecture
    pub fn fetch_checksum(
        &self,
        tool: &str,
        version: &str,
        platform: &str,
        arch: &str,
    ) -> Result<(String, String)> {
        log::info!(
            "Fetching checksum for {} {} ({}/{})",
            tool,
            version,
            platform,
            arch
        );

        match tool {
            "yt-dlp" => self.fetch_yt_dlp_checksum(version, platform, arch),
            "ffmpeg" => self.fetch_ffmpeg_checksum(version, platform, arch),
            "wget" => self.fetch_wget_checksum(version, platform, arch),
            _ => Err(anyhow!("Unknown tool: {}", tool)),
        }
    }

    /// Fetch yt-dlp checksum from GitHub releases
    fn fetch_yt_dlp_checksum(
        &self,
        version: &str,
        platform: &str,
        arch: &str,
    ) -> Result<(String, String)> {
        let repo = "yt-dlp/yt-dlp";
        let api_url = if version == "latest" {
            format!("https://api.github.com/repos/{}/releases/latest", repo)
        } else {
            format!(
                "https://api.github.com/repos/{}/releases/tags/{}",
                repo, version
            )
        };

        log::debug!("Fetching from: {}", api_url);

        let release: GitHubRelease = self
            .client
            .get(&api_url)
            .send()
            .context("Failed to fetch release info")?
            .error_for_status()
            .context("GitHub API request failed")?
            .json()
            .context("Failed to parse release JSON")?;

        // Determine the binary name based on platform
        let binary_name = match (platform, arch) {
            ("windows", "x86_64") => "yt-dlp.exe",
            ("linux", "x86_64") => "yt-dlp_linux",
            ("macos", "x86_64") => "yt-dlp_macos",
            ("macos", "aarch64") => "yt-dlp_macos",
            _ => return Err(anyhow!("Unsupported platform/arch: {}/{}", platform, arch)),
        };

        // Look for SHA256 file
        let checksum_file_name = format!("{}.sha256", binary_name);

        if let Some(asset) = release.assets.iter().find(|a| a.name == checksum_file_name) {
            log::debug!("Found checksum file: {}", asset.name);

            let checksum_content = self
                .client
                .get(&asset.browser_download_url)
                .send()
                .context("Failed to download checksum file")?
                .text()
                .context("Failed to read checksum file")?;

            // Parse checksum (format is usually "hash  filename" or just "hash")
            let hash = checksum_content
                .split_whitespace()
                .next()
                .ok_or_else(|| anyhow!("Invalid checksum file format"))?
                .trim()
                .to_lowercase();

            // Validate SHA256 format (64 hex characters)
            if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(anyhow!("Invalid SHA256 hash format: {}", hash));
            }

            return Ok((hash, api_url));
        }

        // If no .sha256 file, check release body for checksums
        if let Some(body) = &release.body {
            if let Some(hash) = self.extract_checksum_from_text(body, binary_name) {
                log::debug!("Found checksum in release body");
                return Ok((hash, api_url));
            }
        }

        Err(anyhow!(
            "No checksum found for {} {} ({}/{})",
            "yt-dlp",
            version,
            platform,
            arch
        ))
    }

    /// Fetch ffmpeg checksum from BtbN/FFmpeg-Builds GitHub releases
    fn fetch_ffmpeg_checksum(
        &self,
        version: &str,
        platform: &str,
        arch: &str,
    ) -> Result<(String, String)> {
        // FFmpeg builds from BtbN/FFmpeg-Builds
        let repo = "BtbN/FFmpeg-Builds";

        let api_url = if version == "latest" {
            format!("https://api.github.com/repos/{}/releases/latest", repo)
        } else {
            // BtbN uses tags like "autobuild-2024-11-18-12-55"
            // For version "7.1" we need to find the corresponding build
            format!(
                "https://api.github.com/repos/{}/releases/tags/n{}",
                repo, version
            )
        };

        log::debug!("Fetching from: {}", api_url);

        let release: GitHubRelease = self
            .client
            .get(&api_url)
            .send()
            .context("Failed to fetch release info")?
            .error_for_status()
            .context("GitHub API request failed")?
            .json()
            .context("Failed to parse release JSON")?;

        // Look for checksum file (usually SHA256SUMS or similar)
        let checksum_files = ["SHA256SUMS", "checksums.txt", "sha256sum.txt"];

        for checksum_file in &checksum_files {
            if let Some(asset) = release.assets.iter().find(|a| a.name == *checksum_file) {
                log::debug!("Found checksum file: {}", asset.name);

                let checksum_content = self
                    .client
                    .get(&asset.browser_download_url)
                    .send()
                    .context("Failed to download checksum file")?
                    .text()
                    .context("Failed to read checksum file")?;

                // Determine binary name pattern based on platform
                let binary_pattern = match (platform, arch) {
                    ("windows", "x86_64") => "win64-gpl",
                    ("linux", "x86_64") => "linux64-gpl",
                    _ => continue,
                };

                // Parse checksum file to find matching entry
                for line in checksum_content.lines() {
                    if line.contains(binary_pattern) && line.contains(".zip") {
                        if let Some(hash) = line.split_whitespace().next() {
                            let hash = hash.trim().to_lowercase();

                            if hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                                log::debug!("Found checksum: {}", hash);
                                return Ok((hash, api_url));
                            }
                        }
                    }
                }
            }
        }

        // Check release body for checksums
        if let Some(body) = &release.body {
            let binary_pattern = match (platform, arch) {
                ("windows", "x86_64") => "win64",
                ("linux", "x86_64") => "linux64",
                _ => "",
            };

            if !binary_pattern.is_empty() {
                if let Some(hash) = self.extract_checksum_from_text(body, binary_pattern) {
                    log::debug!("Found checksum in release body");
                    return Ok((hash, api_url));
                }
            }
        }

        Err(anyhow!(
            "No checksum found for ffmpeg {} ({}/{})",
            version,
            platform,
            arch
        ))
    }

    /// Fetch wget checksum (wget is less common, may need manual verification)
    fn fetch_wget_checksum(
        &self,
        version: &str,
        platform: &str,
        arch: &str,
    ) -> Result<(String, String)> {
        // wget builds from eternallybored.org don't have an API
        // We'll check if there's a GitHub mirror
        let repo = "mirror/wget";

        let api_url = if version == "latest" {
            format!("https://api.github.com/repos/{}/releases/latest", repo)
        } else {
            format!(
                "https://api.github.com/repos/{}/releases/tags/v{}",
                repo, version
            )
        };

        log::debug!("Fetching from: {}", api_url);

        let release: GitHubRelease = self
            .client
            .get(&api_url)
            .send()
            .context("Failed to fetch release info")?
            .error_for_status()
            .context("GitHub API request failed")?
            .json()
            .context("Failed to parse release JSON")?;

        // Look for checksum files
        for asset in &release.assets {
            if asset.name.ends_with(".sha256") || asset.name.ends_with(".sha256sum") {
                let checksum_content = self
                    .client
                    .get(&asset.browser_download_url)
                    .send()
                    .context("Failed to download checksum file")?
                    .text()
                    .context("Failed to read checksum file")?;

                if let Some(hash) = checksum_content.split_whitespace().next() {
                    let hash = hash.trim().to_lowercase();

                    if hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Ok((hash, api_url));
                    }
                }
            }
        }

        Err(anyhow!(
            "No checksum found for wget {} ({}/{}). Consider using fallback registry.",
            version,
            platform,
            arch
        ))
    }

    /// Extract checksum from text (release body, README, etc.)
    fn extract_checksum_from_text(&self, text: &str, binary_identifier: &str) -> Option<String> {
        // Look for lines containing the binary identifier and a SHA256 hash
        for line in text.lines() {
            if line.contains(binary_identifier) {
                // Try to find a 64-character hex string (SHA256)
                let words: Vec<&str> = line.split_whitespace().collect();
                for word in words {
                    let word = word.trim().to_lowercase();
                    if word.len() == 64 && word.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Some(word);
                    }
                }
            }
        }

        // Also try to find checksum in format "sha256: <hash>"
        if let Some(pos) = text
            .to_lowercase()
            .find(&format!("{}:", binary_identifier.to_lowercase()))
        {
            let after = &text[pos..];
            for word in after.split_whitespace().skip(1) {
                let word = word.trim().to_lowercase();
                if word.len() == 64 && word.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(word);
                }
            }
        }

        None
    }
}

impl Default for ChecksumFetcher {
    fn default() -> Self {
        Self::new().expect("Failed to create default ChecksumFetcher")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_checksum_from_text() {
        let fetcher = ChecksumFetcher::new().unwrap();

        // Valid 64-character SHA256 hash
        let valid_hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

        let text = format!("yt-dlp.exe: {}", valid_hash);
        let hash = fetcher.extract_checksum_from_text(&text, "yt-dlp.exe");
        assert_eq!(hash, Some(valid_hash.to_string()));

        // Test with newlines and extra text
        let text = format!("Download:\nyt-dlp.exe {}\nOther file: 123456", valid_hash);
        let hash = fetcher.extract_checksum_from_text(&text, "yt-dlp.exe");
        assert_eq!(hash, Some(valid_hash.to_string()));
    }

    #[test]
    fn test_extract_checksum_not_found() {
        let fetcher = ChecksumFetcher::new().unwrap();

        let text = "Some random text without checksums";
        let hash = fetcher.extract_checksum_from_text(text, "binary.exe");
        assert_eq!(hash, None);
    }

    // Integration tests (commented out by default as they require network access)
    // Uncomment to test actual fetching

    /*
    #[test]
    #[ignore] // Ignore by default, run with --ignored
    fn test_fetch_yt_dlp_latest() {
        let fetcher = ChecksumFetcher::new().unwrap();
        let result = fetcher.fetch_checksum("yt-dlp", "latest", "windows", "x86_64");

        match result {
            Ok((hash, source)) => {
                println!("Hash: {}", hash);
                println!("Source: {}", source);
                assert_eq!(hash.len(), 64);
                assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
            }
            Err(e) => {
                println!("Warning: Failed to fetch (may be rate limited): {}", e);
            }
        }
    }
    */
}
