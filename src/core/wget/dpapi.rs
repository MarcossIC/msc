// DPAPI (Data Protection API) Windows wrapper for Chrome cookie decryption
// Only available on Windows systems

use anyhow::Result;

#[cfg(windows)]
use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
#[cfg(windows)]
use windows_sys::Win32::Security::Cryptography::{
    CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

/// Decrypt data using Windows DPAPI (Data Protection API)
///
/// DPAPI is a Windows-specific encryption mechanism that ties encrypted data
/// to the user account. This is used by Chrome to encrypt the AES master key
/// stored in the Local State file.
///
/// # Arguments
/// * `encrypted_data` - The DPAPI-encrypted data (without the "DPAPI" prefix)
///
/// # Returns
/// * `Ok(Vec<u8>)` - The decrypted data
/// * `Err(...)` - If DPAPI decryption fails (wrong user, corrupted data, etc.)
///
/// # Platform Support
/// * Windows: Uses CryptUnprotectData API
/// * Other platforms: Returns an error (DPAPI is Windows-only)
#[cfg(windows)]
pub fn decrypt_dpapi(encrypted_data: &[u8]) -> Result<Vec<u8>> {
    use std::ptr::null_mut;

    // Input blob pointing to encrypted data
    let mut input_blob = CRYPT_INTEGER_BLOB {
        cbData: encrypted_data.len() as u32,
        pbData: encrypted_data.as_ptr() as *mut u8,
    };

    // Output blob will be filled by CryptUnprotectData
    let mut output_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    // Call Windows DPAPI to decrypt
    let result = unsafe {
        CryptUnprotectData(
            &mut input_blob,
            null_mut(),                // No description
            null_mut(),                // No additional entropy
            null_mut(),                // Reserved
            null_mut(),                // No prompt struct
            CRYPTPROTECT_UI_FORBIDDEN, // No UI prompts
            &mut output_blob,
        )
    };

    // Check if decryption succeeded
    if result == 0 {
        return Err(anyhow::anyhow!(
            "DPAPI decryption failed. \
            This can occur if:\n\
            • Chrome was installed by a different Windows user\n\
            • Your Windows user profile is corrupted\n\
            • Insufficient permissions\n\n\
            Try:\n\
            1. Close Chrome completely\n\
            2. Run msc from the same user account that uses Chrome\n\
            3. Check Windows Event Viewer for DPAPI errors"
        ));
    }

    // Copy decrypted data before freeing
    let decrypted = unsafe {
        std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec()
    };

    // Free memory allocated by Windows
    unsafe {
        LocalFree(output_blob.pbData as HLOCAL);
    }

    Ok(decrypted)
}

/// Non-Windows platforms: DPAPI is not available
#[cfg(not(windows))]
pub fn decrypt_dpapi(_encrypted_data: &[u8]) -> Result<Vec<u8>> {
    Err(anyhow::anyhow!(
        "DPAPI is only available on Windows.\n\
        Chrome cookie decryption is not supported on this platform.\n\n\
        Alternatives:\n\
        • Use Firefox (cookies stored in plaintext)\n\
        • Export cookies using a browser extension\n\
        • Use a cross-platform tool like py-cookie-cutter"
    ))
}

#[cfg(test)]
#[cfg(windows)]
mod tests {
    use super::*;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN};

    #[test]
    fn test_dpapi_roundtrip() {
        // Test data
        let plaintext = b"test_data_12345_secret_key";

        // Encrypt with DPAPI
        let mut input_blob = CRYPT_INTEGER_BLOB {
            cbData: plaintext.len() as u32,
            pbData: plaintext.as_ptr() as *mut u8,
        };

        let mut output_blob = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        let encrypt_result = unsafe {
            CryptProtectData(
                &mut input_blob,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output_blob,
            )
        };

        assert_ne!(encrypt_result, 0, "DPAPI encryption failed");

        let encrypted = unsafe {
            std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec()
        };

        unsafe {
            LocalFree(output_blob.pbData as HLOCAL);
        }

        // Decrypt with our function
        let decrypted = decrypt_dpapi(&encrypted).expect("DPAPI decryption failed");

        // Verify roundtrip
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_dpapi_with_invalid_data() {
        let invalid_data = b"this_is_not_encrypted_with_dpapi";
        let result = decrypt_dpapi(invalid_data);

        assert!(result.is_err(), "Should fail with invalid DPAPI data");
    }
}
