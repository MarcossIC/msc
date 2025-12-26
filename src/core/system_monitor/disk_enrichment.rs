//! Disk enrichment - platform-specific detailed disk information.
//!
//! This module provides extended disk information (type, SMART data, temperature)
//! by delegating to platform-specific implementations.

use crate::core::system_monitor::metrics::DiskMetrics;
use crate::error::Result;

#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::sync::Mutex;
#[cfg(target_os = "windows")]
use std::time::Instant;

/// Trait for enriching basic disk metrics with platform-specific details.
///
/// Implementations query hardware-specific APIs (PowerShell, WMI, smartctl, etc.)
/// to retrieve extended information like disk type, temperature, and SMART status.
pub trait DiskEnrichmentProvider: Send + Sync {
    /// Enrich a single disk metric with extended information.
    ///
    /// # Arguments
    /// * `disk` - Basic disk metrics from sysinfo
    ///
    /// # Returns
    /// * `Ok(DiskMetrics)` - Enriched metrics with extended fields populated
    /// * `Err(e)` - If enrichment fails, callers should use original disk
    fn enrich_disk(&self, disk: DiskMetrics) -> Result<DiskMetrics>;

    /// Enrich multiple disks in batch (can be optimized for batch queries).
    fn enrich_disks(&self, disks: Vec<DiskMetrics>) -> Vec<DiskMetrics> {
        disks
            .into_iter()
            .map(|disk| self.enrich_disk(disk.clone()).unwrap_or(disk))
            .collect()
    }
}

/// Get the platform-specific disk enrichment provider.
///
/// Returns the appropriate implementation for the current platform:
/// - Windows: Uses PowerShell Get-PhysicalDisk
/// - Linux/macOS: Returns fallback (no enrichment yet)
pub fn get_disk_enrichment_provider() -> Box<dyn DiskEnrichmentProvider> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsDiskEnrichment::new())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Box::new(FallbackDiskEnrichment)
    }
}

// ============================================================================
// Windows Implementation
// ============================================================================

#[cfg(target_os = "windows")]
struct WindowsDiskEnrichment {
    // Cache for disk details to avoid excessive PowerShell calls
    cache: Mutex<HashMap<String, CachedDiskDetails>>,
    cache_ttl_secs: u64,
}

#[cfg(target_os = "windows")]
struct CachedDiskDetails {
    details: crate::platform::system_info_windows::DiskDetailsWindows,
    timestamp: Instant,
}

#[cfg(target_os = "windows")]
impl WindowsDiskEnrichment {
    fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            cache_ttl_secs: 30, // Cache for 30 seconds
        }
    }

    /// Match disk name from sysinfo with PowerShell disk data.
    ///
    /// Uses fuzzy matching because:
    /// - sysinfo provides: "\\.\PhysicalDrive0", "C:", etc.
    /// - PowerShell provides: "Samsung SSD 980 PRO", Model names
    #[allow(dead_code)]
    fn match_disk_name(&self, sysinfo_name: &str, ps_name: &str, ps_model: &str) -> bool {
        // Extract drive number from sysinfo name (e.g., "\\.\PhysicalDrive0" -> "0")
        if let Some(num_str) = sysinfo_name.strip_prefix("\\\\.\\PhysicalDrive") {
            if ps_name.contains(num_str) || ps_model.contains(num_str) {
                return true;
            }
        }

        // Partial string match
        sysinfo_name.contains(ps_name)
            || ps_name.contains(sysinfo_name)
            || sysinfo_name.contains(ps_model)
            || ps_model.contains(sysinfo_name)
    }
}

#[cfg(target_os = "windows")]
impl DiskEnrichmentProvider for WindowsDiskEnrichment {
    fn enrich_disk(&self, mut disk: DiskMetrics) -> Result<DiskMetrics> {
        use crate::platform::system_info_windows::get_disk_details;

        // Check cache first
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(&disk.name) {
                let age = cached.timestamp.elapsed().as_secs();
                if age < self.cache_ttl_secs {
                    // Use cached data
                    let details = &cached.details;
                    disk.disk_type = Some(details.disk_type.clone());
                    disk.manufacturer = details.manufacturer.clone();
                    disk.model = details.model.clone();
                    disk.bus_type = details.bus_type.clone();
                    disk.interface_speed = details.interface_speed.clone();
                    disk.smart_status = details.smart_status.clone();
                    disk.temperature_celsius = details.temperature_celsius;
                    disk.power_on_hours = details.power_on_hours;
                    disk.total_bytes_read = details.total_bytes_read;
                    disk.total_bytes_written = details.total_bytes_written;
                    return Ok(disk);
                } else {
                    // Cache expired, remove it
                    cache.remove(&disk.name);
                }
            }
        }

        // Query Windows for details
        match get_disk_details(&disk.name) {
            Ok(details) => {
                // Populate extended fields
                disk.disk_type = Some(details.disk_type.clone());
                disk.manufacturer = details.manufacturer.clone();
                disk.model = details.model.clone();
                disk.bus_type = details.bus_type.clone();
                disk.interface_speed = details.interface_speed.clone();
                disk.smart_status = details.smart_status.clone();
                disk.temperature_celsius = details.temperature_celsius;
                disk.power_on_hours = details.power_on_hours;
                disk.total_bytes_read = details.total_bytes_read;
                disk.total_bytes_written = details.total_bytes_written;

                // Update cache
                let mut cache = self.cache.lock().unwrap();
                cache.insert(
                    disk.name.clone(),
                    CachedDiskDetails {
                        details,
                        timestamp: Instant::now(),
                    },
                );

                Ok(disk)
            }
            Err(_e) => {
                // Return original disk without extended data
                Ok(disk)
            }
        }
    }
}

// ============================================================================
// Fallback Implementation (Linux/macOS)
// ============================================================================

#[cfg(not(target_os = "windows"))]
struct FallbackDiskEnrichment;

#[cfg(not(target_os = "windows"))]
impl DiskEnrichmentProvider for FallbackDiskEnrichment {
    fn enrich_disk(&self, disk: DiskMetrics) -> Result<DiskMetrics> {
        // No enrichment on unsupported platforms
        Ok(disk)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_fallback_provider() {
        let provider = FallbackDiskEnrichment;
        let disk = DiskMetrics {
            name: "test".to_string(),
            mount_point: "/".to_string(),
            fs_type: "ext4".to_string(),
            total_bytes: 1000,
            available_bytes: 500,
            usage_percent: 50.0,
            ..Default::default()
        };

        let enriched = provider.enrich_disk(disk.clone()).unwrap();
        // Should return unchanged
        assert_eq!(enriched.name, disk.name);
        assert!(enriched.disk_type.is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_provider_cache() {
        let provider = WindowsDiskEnrichment::new();
        // Test cache expiration logic
        assert_eq!(provider.cache_ttl_secs, 30);
    }

    #[test]
    fn test_get_provider_returns_valid_instance() {
        let provider = get_disk_enrichment_provider();
        // Should not panic and should return a valid instance
        let disk = DiskMetrics {
            name: "test".to_string(),
            ..Default::default()
        };
        let result = provider.enrich_disk(disk);
        assert!(result.is_ok());
    }
}
