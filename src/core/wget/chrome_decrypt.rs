// Chrome cookie decryption module
// Handles AES-256-GCM decryption of Chrome/Edge/Brave encrypted cookies

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::path::PathBuf;

use super::dpapi::decrypt_dpapi;

/// Encryption version prefixes used by Chrome
const ENCRYPTION_PREFIX_V10: &[u8] = b"v10";
const ENCRYPTION_PREFIX_V11: &[u8] = b"v11";
const ENCRYPTION_PREFIX_V20: &[u8] = b"v20"; // App-Bound Encryption
const DPAPI_PREFIX: &[u8] = b"DPAPI";

/// Chrome cookie decryptor
///
/// Manages the decryption of Chrome-based browser cookies.
/// Chrome 80+ encrypts cookie values using DPAPI + AES-256-GCM.
///
/// Decryption flow:
/// 1. Read Local State JSON file
/// 2. Extract and decode encrypted_key
/// 3. Remove "DPAPI" prefix
/// 4. Decrypt with Windows DPAPI → Get AES-256 key
/// 5. For each cookie: Extract version prefix, nonce, and ciphertext
/// 6. Decrypt with AES-256-GCM → Get plaintext cookie value
pub struct ChromeDecryptor {
    /// AES-256 key extracted from Local State (DPAPI-decrypted)
    aes_key: Option<Vec<u8>>,
}

impl ChromeDecryptor {
    /// Create a new ChromeDecryptor for a specific browser
    ///
    /// # Arguments
    /// * `browser` - Browser name: "chrome", "edge", "brave", or "chromium"
    ///
    /// # Returns
    /// * `Ok(ChromeDecryptor)` - Successfully initialized with AES key
    /// * `Err(...)` - Failed to find Local State or extract key
    pub fn new(browser: &str) -> Result<Self> {
        let local_state_path = Self::get_local_state_path(browser)?;
        let aes_key = Self::extract_aes_key(&local_state_path, browser)?;

        Ok(Self {
            aes_key
        })
    }

    /// Find the Local State file path for a given browser
    ///
    /// # Browser Paths (Windows)
    /// - Chrome: `%LOCALAPPDATA%\Google\Chrome\User Data\Local State`
    /// - Edge: `%LOCALAPPDATA%\Microsoft\Edge\User Data\Local State`
    /// - Brave: `%LOCALAPPDATA%\BraveSoftware\Brave-Browser\User Data\Local State`
    /// - Chromium: `%LOCALAPPDATA%\Chromium\User Data\Local State`
    fn get_local_state_path(browser: &str) -> Result<PathBuf> {
        let local_app_data =
            std::env::var("LOCALAPPDATA").context("LOCALAPPDATA environment variable not set")?;

        let base_path = PathBuf::from(local_app_data);

        let local_state = match browser.to_lowercase().as_str() {
            "chrome" | "google-chrome" => base_path.join("Google/Chrome/User Data/Local State"),
            "edge" | "ms-edge" | "microsoft-edge" => {
                base_path.join("Microsoft/Edge/User Data/Local State")
            }
            "brave" | "brave-browser" => {
                base_path.join("BraveSoftware/Brave-Browser/User Data/Local State")
            }
            "chromium" => base_path.join("Chromium/User Data/Local State"),
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported browser: {}\nSupported: chrome, edge, brave, chromium",
                    browser
                ))
            }
        };

        if !local_state.exists() {
            return Err(anyhow::anyhow!(
                "Local State file not found: {}\n\n\
                Possible causes:\n\
                • {} is not installed\n\
                • You have never run {}\n\
                • Incorrect user profile\n\n\
                Verify that '{}' exists",
                local_state.display(),
                browser,
                browser,
                base_path.display()
            ));
        }

        Ok(local_state)
    }

    /// Extract and decrypt the AES-256 key from Local State file
    ///
    /// # Arguments
    /// * `local_state_path` - Path to the Local State JSON file
    /// * `browser` - Browser name (for error messages)
    ///
    /// # Returns
    /// * `Ok(Some(Vec<u8>))` - Successfully extracted AES key
    /// * `Ok(None)` - No encryption (very old Chrome or App-Bound)
    /// * `Err(...)` - Failed to read or decrypt
    fn extract_aes_key(local_state_path: &PathBuf, browser: &str) -> Result<Option<Vec<u8>>> {
        // Read Local State JSON
        let content = std::fs::read_to_string(local_state_path)
            .context("Failed to read Local State file")?;

        let json: serde_json::Value =
            serde_json::from_str(&content).context("Local State is not valid JSON")?;

        // Check for App-Bound Encryption (Chrome 127+)
        if json
            .get("os_crypt")
            .and_then(|o| o.get("app_bound_encrypted_key"))
            .is_some()
        {
            eprintln!();
            eprintln!("⚠️  Chrome 127+ App-Bound Encryption detected.");
            eprintln!("   This version requires SYSTEM-level privileges to decrypt.");
            eprintln!();
            eprintln!("Solutions:");
            eprintln!("  1. Use Firefox instead (cookies stored in plaintext)");
            eprintln!("  2. Export cookies using a browser extension");
            eprintln!("  3. Disable App-Bound Encryption (requires admin):");
            eprintln!("     reg add \"HKLM\\Software\\Policies\\Google\\Chrome\" \\");
            eprintln!("         /v ApplicationBoundEncryptionEnabled /t REG_DWORD /d 0 /f");
            eprintln!();

            return Err(anyhow::anyhow!(
                "App-Bound Encryption not supported (Chrome 127+)"
            ));
        }

        // Try to get standard encrypted_key
        if let Some(encrypted_key_b64) = json
            .get("os_crypt")
            .and_then(|o| o.get("encrypted_key"))
            .and_then(|k| k.as_str())
        {
            // Decode from Base64
            let encrypted_key = STANDARD
                .decode(encrypted_key_b64)
                .context("Failed to decode encrypted_key from Base64")?;

            // Check for DPAPI prefix
            if encrypted_key.starts_with(DPAPI_PREFIX) {
                let key_without_prefix = &encrypted_key[DPAPI_PREFIX.len()..];

                // Decrypt with DPAPI
                let decrypted_key = decrypt_dpapi(key_without_prefix).context(format!(
                    "Failed to decrypt AES key with DPAPI for {}",
                    browser
                ))?;

                return Ok(Some(decrypted_key));
            } else {
                return Err(anyhow::anyhow!(
                    "encrypted_key does not have DPAPI prefix.\n\
                    Expected prefix: {:?}, Found: {:?}",
                    DPAPI_PREFIX,
                    &encrypted_key[..DPAPI_PREFIX.len().min(encrypted_key.len())]
                ));
            }
        }

        // No encrypted_key found (very old Chrome or unusual setup)
        Ok(None)
    }

    /// Decrypt a cookie's encrypted_value field
    ///
    /// # Arguments
    /// * `encrypted_value` - Raw bytes from the encrypted_value column
    ///
    /// # Returns
    /// * `Ok(String)` - Decrypted cookie value
    /// * `Err(...)` - Decryption failed
    ///
    /// # Encryption Versions
    /// - v10/v11: Standard AES-256-GCM encryption
    /// - v20: App-Bound Encryption (returns error)
    /// - No prefix: Legacy plaintext or old DPAPI
    pub fn decrypt_cookie_value(&self, encrypted_value: &[u8]) -> Result<String> {
        // If no AES key, try legacy decryption
        let aes_key = match &self.aes_key {
            Some(key) => key,
            None => {
                return self.try_legacy_decrypt(encrypted_value);
            }
        };

        // Check minimum length (3 byte prefix + 12 byte nonce + data)
        if encrypted_value.len() < 15 {
            return Err(anyhow::anyhow!(
                "Encrypted value too short: {} bytes (minimum 15 expected)",
                encrypted_value.len()
            ));
        }

        // Split prefix and encrypted data
        let (version_prefix, encrypted_data) = encrypted_value.split_at(3);

        // Route based on version
        match version_prefix {
            ENCRYPTION_PREFIX_V10 | ENCRYPTION_PREFIX_V11 => {
                self.decrypt_aes_gcm(aes_key, encrypted_data)
            }
            ENCRYPTION_PREFIX_V20 => Err(anyhow::anyhow!(
                "v20 (App-Bound Encryption) is not supported.\n\
                See error message above for solutions."
            )),
            _ => {
                // Try legacy decryption (no version prefix)
                self.try_legacy_decrypt(encrypted_value)
            }
        }
    }

    /// Decrypt using AES-256-GCM
    ///
    /// # Chrome Encrypted Value Structure (after version prefix):
    /// ```text
    /// [12 bytes: nonce/IV][variable: ciphertext + 16 byte auth tag]
    /// ```
    ///
    /// # Arguments
    /// * `key` - 32-byte AES-256 key
    /// * `encrypted_data` - Nonce + ciphertext (without version prefix)
    fn decrypt_aes_gcm(&self, key: &[u8], encrypted_data: &[u8]) -> Result<String> {
        // Minimum: 12 bytes nonce + 16 bytes tag
        if encrypted_data.len() < 28 {
            return Err(anyhow::anyhow!(
                "Encrypted data too short for AES-GCM: {} bytes (minimum 28)",
                encrypted_data.len()
            ));
        }

        // Split nonce and ciphertext
        let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);

        // Create AES-256-GCM cipher
        let cipher = Aes256Gcm::new_from_slice(key).context("Failed to create AES-256-GCM cipher (invalid key length)")?;

        // Create nonce from first 12 bytes
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let decrypted = cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("AES-GCM decryption failed: {:?}\n\nPossible causes:\n• Cookie was encrypted by a different browser profile\n• Database corruption\n• Wrong AES key", e))?;

        // Convert to UTF-8 string
        String::from_utf8(decrypted).context("Decrypted cookie value is not valid UTF-8")
    }

    /// Try legacy decryption methods for old Chrome versions
    ///
    /// # Attempts:
    /// 1. Interpret as plaintext UTF-8 (very old Chrome)
    /// 2. Direct DPAPI decryption (Chrome < 80 on Windows)
    fn try_legacy_decrypt(&self, data: &[u8]) -> Result<String> {
        // Attempt 1: Try as plaintext
        if let Ok(text) = String::from_utf8(data.to_vec()) {
            // Check if it looks like valid text (no excessive control characters)
            if text.chars().all(|c| !c.is_control() || c == '\n' || c == '\t') {
                return Ok(text);
            }
        }

        // Attempt 2: Try direct DPAPI decryption (Chrome < 80)
        #[cfg(windows)]
        if let Ok(decrypted) = decrypt_dpapi(data) {
            if let Ok(text) = String::from_utf8(decrypted) {
                return Ok(text);
            }
        }

        Err(anyhow::anyhow!(
            "Could not decrypt cookie value.\n\
            Not plaintext, not v10/v11 encrypted, and DPAPI decryption failed."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_prefix_detection() {
        let v10_data = b"v10";
        let v11_data = b"v11";
        let v20_data = b"v20";

        assert_eq!(v10_data, ENCRYPTION_PREFIX_V10);
        assert_eq!(v11_data, ENCRYPTION_PREFIX_V11);
        assert_eq!(v20_data, ENCRYPTION_PREFIX_V20);
    }

    #[test]
    fn test_dpapi_prefix() {
        let dpapi_bytes = b"DPAPI";
        assert_eq!(dpapi_bytes, DPAPI_PREFIX);
    }

    #[test]
    fn test_chrome_time_conversion_example() {
        // Chrome time for 2024-01-01 00:00:00 UTC
        // Unix: 1704067200 seconds since 1970
        // Chrome: (1704067200 * 1000000) + 11644473600000000 = 13348540800000000
        // Formula: (chrome_time - 11644473600000000) / 1000000
        const CHROME_EPOCH_OFFSET: i64 = 11644473600000000;
        let chrome_time = 13348540800000000i64;
        let unix_time = (chrome_time - CHROME_EPOCH_OFFSET) / 1000000;
        assert_eq!(unix_time, 1704067200);
    }

    #[test]
    fn test_encryption_prefix_sizes() {
        // Verify all prefixes are exactly 3 bytes
        assert_eq!(ENCRYPTION_PREFIX_V10.len(), 3);
        assert_eq!(ENCRYPTION_PREFIX_V11.len(), 3);
        assert_eq!(ENCRYPTION_PREFIX_V20.len(), 3);
        assert_eq!(DPAPI_PREFIX.len(), 5);
    }

    #[test]
    fn test_get_local_state_path_chrome() {
        // Test that Chrome path is constructed correctly
        if let Ok(_local_app_data) = std::env::var("LOCALAPPDATA") {
            let result = ChromeDecryptor::get_local_state_path("chrome");

            if let Ok(path) = result {
                let path_str = path.to_string_lossy();
                assert!(path_str.contains("Google"));
                assert!(path_str.contains("Chrome"));
                assert!(path_str.contains("User Data"));
                assert!(path_str.ends_with("Local State"));
            }
            // If Chrome is not installed, it's ok for this test to not assert
        }
    }

    #[test]
    fn test_get_local_state_path_edge() {
        // Test that Edge path is constructed correctly
        if let Ok(_local_app_data) = std::env::var("LOCALAPPDATA") {
            let result = ChromeDecryptor::get_local_state_path("edge");

            if let Ok(path) = result {
                let path_str = path.to_string_lossy();
                assert!(path_str.contains("Microsoft"));
                assert!(path_str.contains("Edge"));
                assert!(path_str.contains("User Data"));
                assert!(path_str.ends_with("Local State"));
            }
        }
    }

    #[test]
    fn test_get_local_state_path_brave() {
        // Test that Brave path is constructed correctly
        if let Ok(_local_app_data) = std::env::var("LOCALAPPDATA") {
            let result = ChromeDecryptor::get_local_state_path("brave");

            if let Ok(path) = result {
                let path_str = path.to_string_lossy();
                assert!(path_str.contains("BraveSoftware"));
                assert!(path_str.contains("Brave-Browser"));
            }
        }
    }

    #[test]
    fn test_get_local_state_path_unsupported() {
        // Test that unsupported browsers return an error
        let result = ChromeDecryptor::get_local_state_path("firefox");

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Unsupported browser"));
        assert!(err_msg.contains("firefox"));
    }

    #[test]
    fn test_browser_name_variations() {
        // Test various browser name formats are accepted
        let browsers = vec![
            ("chrome", true),
            ("google-chrome", true),
            ("edge", true),
            ("ms-edge", true),
            ("microsoft-edge", true),
            ("brave", true),
            ("brave-browser", true),
            ("chromium", true),
            ("firefox", false), // Not supported
            ("safari", false),  // Not supported
        ];

        for (browser, should_work) in browsers {
            let result = ChromeDecryptor::get_local_state_path(browser);

            if should_work {
                // Should either succeed or fail because browser not installed
                // Either way, it shouldn't fail with "unsupported browser"
                if let Err(e) = result {
                    let err_msg = e.to_string();
                    assert!(
                        !err_msg.contains("Unsupported browser"),
                        "Browser '{}' should be supported but got: {}",
                        browser,
                        err_msg
                    );
                }
            } else {
                // Should fail with "unsupported browser"
                assert!(result.is_err());
                let err_msg = result.unwrap_err().to_string();
                assert!(err_msg.contains("Unsupported browser"));
            }
        }
    }

    #[test]
    fn test_decrypt_cookie_value_too_short() {
        // Test that very short encrypted values are rejected
        let decryptor = ChromeDecryptor {
            aes_key: Some(vec![0u8; 32]), // Dummy key
        };

        let too_short = b"v10";
        let result = decryptor.decrypt_cookie_value(too_short);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("too short"));
    }

    #[test]
    fn test_decrypt_cookie_value_v20_rejection() {
        // Test that v20 (App-Bound Encryption) is rejected
        let decryptor = ChromeDecryptor {
            aes_key: Some(vec![0u8; 32]),
        };

        // Create v20 data: "v20" prefix + dummy encrypted data
        let mut v20_data = Vec::new();
        v20_data.extend_from_slice(b"v20");
        v20_data.extend_from_slice(&[0u8; 20]); // Add dummy data

        let result = decryptor.decrypt_cookie_value(&v20_data);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("v20"));
        assert!(err_msg.contains("App-Bound Encryption"));
    }

    #[test]
    fn test_decrypt_cookie_value_no_key_legacy() {
        // Test legacy decryption when no AES key is present
        let decryptor = ChromeDecryptor {
            aes_key: None,
        };

        // Try plaintext
        let plaintext = b"plaintext_cookie_value";
        let result = decryptor.decrypt_cookie_value(plaintext);

        // Should either succeed as plaintext or fail with legacy decrypt error
        if let Ok(value) = result {
            assert_eq!(value, "plaintext_cookie_value");
        }
    }

    #[test]
    fn test_try_legacy_decrypt_plaintext() {
        // Test that plaintext values are handled correctly
        let decryptor = ChromeDecryptor {
            aes_key: None,
        };

        let plaintext = b"simple_value";
        let result = decryptor.try_legacy_decrypt(plaintext);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "simple_value");
    }

    #[test]
    fn test_try_legacy_decrypt_invalid() {
        // Test that invalid binary data is rejected
        let decryptor = ChromeDecryptor {
            aes_key: None,
        };

        // Invalid UTF-8 and not DPAPI-encrypted
        let invalid = &[0xFF, 0xFE, 0xFD, 0xFC, 0xFB];
        let result = decryptor.try_legacy_decrypt(invalid);

        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_aes_gcm_too_short() {
        // Test that encrypted data too short for AES-GCM is rejected
        let decryptor = ChromeDecryptor {
            aes_key: Some(vec![0u8; 32]),
        };

        let too_short = &[0u8; 20]; // Less than 28 bytes required
        let result = decryptor.decrypt_aes_gcm(&vec![0u8; 32], too_short);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("too short"));
        assert!(err_msg.contains("AES-GCM"));
    }

    #[test]
    #[ignore] // Requires actual Chrome installation
    fn test_new_with_real_chrome() {
        // Integration test - only runs if Chrome is installed
        let result = ChromeDecryptor::new("chrome");

        // Should either succeed or fail with a clear error message
        match result {
            Ok(_decryptor) => {
                // If successful, should have AES key (unless very old Chrome)
                println!("Successfully created decryptor");
            }
            Err(e) => {
                let err_msg = e.to_string();
                // Error should be informative
                assert!(
                    err_msg.contains("Local State") ||
                    err_msg.contains("not found") ||
                    err_msg.contains("App-Bound"),
                    "Error should be informative: {}",
                    err_msg
                );
            }
        }
    }
}
