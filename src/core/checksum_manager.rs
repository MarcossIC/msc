use anyhow::{ensure, Result};
use sha2::{Sha256, Digest};
use std::fs::File;
use std::io;
use std::path::Path;

/// Known good checksums for downloaded binaries
/// TODO: Update these with actual checksums from official sources
pub struct ChecksumManager;

impl ChecksumManager {
    /// Verify file matches expected SHA256 hash
    pub fn verify_file(path: &Path, expected_hash: &str) -> Result<()> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();

        io::copy(&mut file, &mut hasher)?;

        let hash = format!("{:x}", hasher.finalize());

        ensure!(
            hash == expected_hash,
            "Hash mismatch!\n  Expected: {}\n  Got:      {}",
            expected_hash,
            hash
        );

        Ok(())
    }

    /// Get expected hash for a specific tool and version
    /// In production, this should fetch from a trusted source or be hardcoded
    pub fn get_expected_hash(tool: &str, version: &str) -> Option<String> {
        // TODO: Implement actual hash lookup
        // For now, return None to indicate verification not yet implemented
        //
        // Future implementation options:
        // 1. Hardcoded hash map for known versions
        // 2. Fetch from official project release page
        // 3. Maintain a trusted hash database file
        //
        // Example:
        // match (tool, version) {
        //     ("yt-dlp", "2024.01.01") => Some("abc123...".to_string()),
        //     ("ffmpeg", "6.0") => Some("def456...".to_string()),
        //     _ => None,
        // }

        let _ = (tool, version); // Suppress unused warnings
        None
    }

    /// Calculate SHA256 hash of a file
    /// Useful for generating hashes of newly downloaded files
    pub fn calculate_hash(path: &Path) -> Result<String> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();

        io::copy(&mut file, &mut hasher)?;

        Ok(format!("{:x}", hasher.finalize()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
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
    fn test_get_expected_hash_not_implemented() {
        let result = ChecksumManager::get_expected_hash("yt-dlp", "2024.01.01");
        assert!(result.is_none(), "Should return None as not yet implemented");
    }
}
