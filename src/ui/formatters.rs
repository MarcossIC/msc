use chrono::{DateTime, Local};
use std::fs;
use std::time::SystemTime;

/// Format file size in human-readable format (B, KB, MB, GB)
pub fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{}B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1}KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.1}MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Format timestamp in human-readable format (YYYY-MM-DD HH:MM)
pub fn format_time(time: SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

/// Format file permissions based on platform
#[cfg(windows)]
pub fn format_permissions(metadata: &fs::Metadata) -> String {
    use std::os::windows::fs::MetadataExt;

    let attributes = metadata.file_attributes();
    let mut perms = String::new();

    if (attributes & 1) != 0 {
        perms.push('R');
    } else {
        perms.push('r');
    } // Read-only
    if (attributes & 2) != 0 {
        perms.push('H');
    } else {
        perms.push('-');
    } // Hidden
    if (attributes & 4) != 0 {
        perms.push('S');
    } else {
        perms.push('-');
    } // System
    if (attributes & 16) != 0 {
        perms.push('D');
    } else {
        perms.push('-');
    } // Directory
    if (attributes & 32) != 0 {
        perms.push('A');
    } else {
        perms.push('-');
    } // Archive

    perms
}

/// Format file permissions based on platform (Unix)
#[cfg(unix)]
pub fn format_permissions(metadata: &fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;

    let mode = metadata.permissions().mode();
    let mut perms = String::new();

    // Owner permissions
    perms.push(if mode & 0o400 != 0 { 'r' } else { '-' });
    perms.push(if mode & 0o200 != 0 { 'w' } else { '-' });
    perms.push(if mode & 0o100 != 0 { 'x' } else { '-' });

    // Group permissions
    perms.push(if mode & 0o040 != 0 { 'r' } else { '-' });
    perms.push(if mode & 0o020 != 0 { 'w' } else { '-' });
    perms.push(if mode & 0o010 != 0 { 'x' } else { '-' });

    // Other permissions
    perms.push(if mode & 0o004 != 0 { 'r' } else { '-' });
    perms.push(if mode & 0o002 != 0 { 'w' } else { '-' });
    perms.push(if mode & 0o001 != 0 { 'x' } else { '-' });

    perms
}
