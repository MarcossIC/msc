use crate::core::system_info::types::{DiskType, StorageInfo};
use crate::error::Result;
use std::time::{Duration, Instant};
use sysinfo::Disks;

#[cfg(windows)]
use crate::platform::system_info_windows;

pub fn collect() -> Result<Vec<StorageInfo>> {
    collect_with_subs().map(|(info, _)| info)
}

/// Storage info paired with per-disk collection timings.
type StorageWithSubs = (Vec<StorageInfo>, Vec<(String, Duration)>);

/// Collect storage info AND per-disk timings.
pub fn collect_with_subs() -> Result<StorageWithSubs> {
    let disks = Disks::new_with_refreshed_list();
    let mut storage = Vec::new();
    let mut subs: Vec<(String, Duration)> = Vec::new();

    for disk in disks.list() {
        let disk_t = Instant::now();

        let total = disk.total_space();
        let available = disk.available_space();
        let used = total.saturating_sub(available);
        let usage_percent = if total > 0 {
            (used as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        let disk_name = disk.name().to_string_lossy().to_string();
        let mount_point = disk.mount_point().to_string_lossy().to_string();

        // Get detailed disk information (type, manufacturer, SMART data, etc.)
        // Use mount point instead of disk name as it's more reliable
        #[cfg(windows)]
        let disk_details = {
            let disk_identifier = if !mount_point.is_empty() {
                &mount_point
            } else {
                &disk_name
            };
            let result = system_info_windows::get_disk_details(disk_identifier);
            result.ok()
        };

        #[cfg(not(windows))]
        let _disk_details: Option<()> = None;

        // Extract info from disk_details if available
        #[cfg(windows)]
        let (
            disk_type,
            manufacturer,
            model,
            serial_number,
            firmware_version,
            bus_type,
            interface_speed,
            smart_status,
            temperature_celsius,
            power_on_hours,
            total_bytes_read,
            total_bytes_written,
        ) = {
            if let Some(ref details) = disk_details {
                (
                    details.disk_type.clone(),
                    details.manufacturer.clone(),
                    details.model.clone(),
                    details.serial_number.clone(),
                    details.firmware_version.clone(),
                    details.bus_type.clone(),
                    details.interface_speed.clone(),
                    details.smart_status.clone(),
                    details.temperature_celsius,
                    details.power_on_hours,
                    details.total_bytes_read,
                    details.total_bytes_written,
                )
            } else {
                // Fallback to basic detection
                (
                    detect_disk_type(&disk_name),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
        };

        #[cfg(not(windows))]
        let (
            disk_type,
            manufacturer,
            model,
            serial_number,
            firmware_version,
            bus_type,
            interface_speed,
            smart_status,
            temperature_celsius,
            power_on_hours,
            total_bytes_read,
            total_bytes_written,
        ) = {
            (
                detect_disk_type(&disk_name),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
        };

        let label = mount_point
            .chars()
            .take(8)
            .collect::<String>()
            .replace(['\\', ':'], "");
        let label = if label.is_empty() {
            format!("disk{}", subs.len())
        } else {
            label
        };
        subs.push((format!("storage.{}", label), disk_t.elapsed()));

        storage.push(StorageInfo {
            name: disk_name,
            mount_point: disk.mount_point().to_string_lossy().to_string(),
            total_bytes: total,
            available_bytes: available,
            used_bytes: used,
            usage_percent,
            disk_type,
            file_system: disk.file_system().to_string_lossy().to_string(),
            manufacturer,
            model,
            serial_number,
            firmware_version,
            bus_type,
            interface_speed,
            smart_status,
            temperature_celsius,
            power_on_hours,
            total_bytes_read,
            total_bytes_written,
        });
    }

    Ok((storage, subs))
}

fn detect_disk_type(name: &str) -> DiskType {
    let name_lower = name.to_lowercase();

    if name_lower.contains("nvme") {
        DiskType::NVMe
    } else if name_lower.contains("ssd") {
        DiskType::SSD
    } else if name_lower.contains("hdd") || name_lower.contains("hard") {
        DiskType::HDD
    } else {
        DiskType::Unknown
    }
}
