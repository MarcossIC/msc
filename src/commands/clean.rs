// Clean command module - Manages cleanup operations and clean paths configuration
//
// This module provides subcommands for:
// - start: Execute cleanup of temporary files
// - add: Add custom clean paths
// - list: List all clean paths (default + custom)
// - remove: Remove custom clean paths
// - clear: Clear all custom clean paths

use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use crate::core::{Config, TempCleaner};
use crate::platform::get_recycle_bin_directory;
#[cfg(windows)]
use crate::platform::{elevate_and_rerun, is_elevated};
use crate::ui::{format_size, read_confirmation, read_exact_confirmation, select_from_list};

/// Categorizes directories by whether they require admin privileges
#[derive(Debug)]
struct DirectoriesByPrivilege {
    user_directories: Vec<String>,
    system_directories: Vec<String>,
}

/// Check if a directory is a user directory (doesn't require admin privileges)
fn is_user_directory(path: &str) -> bool {
    let path_lower = path.to_lowercase();

    // Get current user's username to check for user-specific paths
    let current_user = std::env::var("USERNAME")
        .ok()
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_default()
        .to_lowercase();

    // User directories that don't require admin
    let user_indicators = [
        &format!("\\users\\{}\\appdata", current_user),
        &format!("\\users\\{}\\temp", current_user),
        &format!("/home/{}/", current_user),
        "\\appdata\\local\\temp",
        "\\appdata\\roaming\\temp",
    ];

    // Check if path contains user directory indicators
    for indicator in &user_indicators {
        if path_lower.contains(indicator) {
            return true;
        }
    }

    // Also check environment-based temp directories
    if let Ok(user_temp) = std::env::var("TEMP") {
        if path.starts_with(&user_temp) {
            return true;
        }
    }

    if let Ok(user_tmp) = std::env::var("TMP") {
        if path.starts_with(&user_tmp) {
            return true;
        }
    }

    false
}

/// Categorize directories by privilege requirements
fn categorize_by_privilege(directories: &[String]) -> DirectoriesByPrivilege {
    let mut user_directories = Vec::new();
    let mut system_directories = Vec::new();

    for dir in directories {
        if is_user_directory(dir) {
            user_directories.push(dir.clone());
        } else {
            system_directories.push(dir.clone());
        }
    }

    DirectoriesByPrivilege {
        user_directories,
        system_directories,
    }
}

/// Helper function to perform cleanup on a specific set of directories
fn perform_cleanup(
    directories: Vec<String>,
    dry_run: bool,
    min_age: Option<std::time::Duration>,
    max_age: Option<std::time::Duration>,
    min_age_hours: Option<&u64>,
    include_recent: bool,
    cancel_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Result<crate::core::CleanupStats> {
    // Create a temporary cleaner with only these directories
    let mut cleaner = TempCleaner::new()?;
    cleaner.directories = directories;
    cleaner.min_age = min_age;
    cleaner.max_age = max_age;

    // Use provided cancel flag if available
    if let Some(flag) = cancel_flag {
        cleaner.cancel_flag = flag;
    }

    // Scan files
    println!("{}", "Scanning temporary files...".dimmed());
    let scan_stats = cleaner.scan();

    // Warn about inaccessible directories early
    if !scan_stats.inaccessible_dirs.is_empty() {
        println!();
        println!(
            "{}",
            format!(
                "⚠️  {} director{} could not be accessed (will be skipped)",
                scan_stats.inaccessible_dirs.len(),
                if scan_stats.inaccessible_dirs.len() == 1 {
                    "y"
                } else {
                    "ies"
                }
            )
            .yellow()
        );
        println!();
    }

    if scan_stats.total_files == 0 {
        if scan_stats.inaccessible_dirs.is_empty() {
            println!("{}", "No temporary files found to clean.".green());
        } else {
            println!(
                "{}",
                "No accessible temporary files found to clean.".yellow()
            );
        }
        return Ok(scan_stats);
    }

    println!(
        "{} {} files ({}) found",
        "Found:".white().bold(),
        scan_stats.total_files.to_string().yellow().bold(),
        format_size(scan_stats.total_size).yellow().bold()
    );
    println!();

    if dry_run {
        println!("{}", "Files that would be deleted:".white().bold());
        println!();
    } else {
        println!("{}", "Cleaning...".cyan().bold());
        println!();
    }

    // Clean with progress callback
    let stats = cleaner.clean(dry_run, |processed, total| {
        let percentage = (processed as f64 / total as f64 * 100.0) as usize;
        let bar_length: usize = 30;
        let filled = (percentage as f64 / 100.0 * bar_length as f64) as usize;
        let empty = bar_length.saturating_sub(filled);

        print!(
            "\r{} [{}{}] {}% ({}/{}) ",
            "Progress:".white(),
            "=".repeat(filled).green(),
            " ".repeat(empty),
            percentage,
            processed,
            total
        );

        use std::io::Write;
        std::io::stdout().flush().ok();
    })?;

    println!();
    println!();

    // Display summary
    display_cleanup_summary(&stats, dry_run, min_age_hours, include_recent);

    Ok(stats)
}

/// Clean work cache directories (target, dist, node_modules) in work directory projects
fn clean_work_cache(config: &crate::core::Config, dry_run: bool) -> Result<()> {
    use std::fs;
    use std::path::PathBuf;

    // Get work path from config
    let work_path = match config.get_work_path() {
        Some(path) => path,
        None => {
            println!("{}", "No work directory configured.".yellow());
            println!(
                "{}",
                "Use 'msc set work <path>' to configure a work directory.".dimmed()
            );
            return Ok(());
        }
    };

    let work_path_buf = PathBuf::from(work_path);
    if !work_path_buf.exists() {
        println!(
            "{}",
            format!("Work directory does not exist: {}", work_path).red()
        );
        return Ok(());
    }

    if !work_path_buf.is_dir() {
        println!(
            "{}",
            format!("Work path is not a directory: {}", work_path).red()
        );
        return Ok(());
    }

    println!("{}", "═".repeat(50).cyan());
    println!("{}", "WORK CACHE CLEANUP".cyan().bold());
    println!("{}", "═".repeat(50).cyan());
    println!();

    // Get ignored folders
    let ignored_folders = config.get_ignored_work_folders();

    if !ignored_folders.is_empty() {
        println!("{}", "Ignored project folders:".dimmed());
        for folder in &ignored_folders {
            println!("  • {}", folder.dimmed());
        }
        println!();
    }

    // Cache folders to clean
    let cache_folders = ["target", "dist", "node_modules"];

    println!(
        "{}",
        "Scanning work directory for project cache folders...".dimmed()
    );
    println!();

    // Scan work directory
    let entries = match fs::read_dir(&work_path_buf) {
        Ok(entries) => entries,
        Err(e) => {
            println!("{}", format!("Error reading work directory: {}", e).red());
            return Ok(());
        }
    };

    let mut total_size: u64 = 0;
    let mut total_files: usize = 0;
    let mut cleaned_count: usize = 0;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let project_path = entry.path();

        // Skip if not a directory
        if !project_path.is_dir() {
            continue;
        }

        // Get folder name
        let folder_name = match project_path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        // Skip if folder is in ignore list
        if ignored_folders.contains(&folder_name) {
            continue;
        }

        // Check for cache folders in this project
        for cache_folder in &cache_folders {
            let cache_path = project_path.join(cache_folder);

            if cache_path.exists() && cache_path.is_dir() {
                // Calculate size
                let (folder_size, file_count) = calculate_dir_size(&cache_path);

                if dry_run {
                    println!(
                        "{} {} in {}/{}",
                        "Would delete:".yellow(),
                        format_size(folder_size).yellow().bold(),
                        folder_name.cyan(),
                        cache_folder.cyan().bold()
                    );
                } else {
                    print!(
                        "{} {}",
                        "Deleting:".cyan(),
                        format!("{}/{}", folder_name, cache_folder).cyan()
                    );

                    // Delete the folder
                    match fs::remove_dir_all(&cache_path) {
                        Ok(_) => {
                            println!(" {} ({})", "✓".green(), format_size(folder_size).dimmed());
                            cleaned_count += 1;
                        }
                        Err(e) => {
                            println!(" {} ({})", "✗".red(), e.to_string().red());
                        }
                    }
                }

                total_size += folder_size;
                total_files += file_count;
            }
        }
    }

    println!();
    println!("{}", "═".repeat(50).cyan());

    if dry_run {
        println!(
            "{} {} cache folders ({} files, {})",
            "Would delete:".yellow().bold(),
            cleaned_count.to_string().yellow().bold(),
            total_files.to_string().dimmed(),
            format_size(total_size).yellow().bold()
        );
    } else {
        println!(
            "{} {} cache folders ({} files, {})",
            "Deleted:".green().bold(),
            cleaned_count.to_string().yellow().bold(),
            total_files.to_string().dimmed(),
            format_size(total_size).yellow().bold()
        );
    }

    println!("{}", "═".repeat(50).cyan());
    println!();

    Ok(())
}

/// Calculate total size and file count of a directory recursively
fn calculate_dir_size(path: &std::path::Path) -> (u64, usize) {
    use std::fs;

    let mut total_size: u64 = 0;
    let mut file_count: usize = 0;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();

            if entry_path.is_file() {
                if let Ok(metadata) = fs::metadata(&entry_path) {
                    total_size += metadata.len();
                    file_count += 1;
                }
            } else if entry_path.is_dir() {
                let (dir_size, dir_files) = calculate_dir_size(&entry_path);
                total_size += dir_size;
                file_count += dir_files;
            }
        }
    }

    (total_size, file_count)
}

/// Display cleanup summary
fn display_cleanup_summary(
    stats: &crate::core::CleanupStats,
    dry_run: bool,
    min_age_hours: Option<&u64>,
    include_recent: bool,
) {
    // Show cancellation status if cancelled
    if stats.was_cancelled {
        println!();
        println!("{}", "═".repeat(50).yellow());
        println!(
            "{}",
            "  OPERATION CANCELLED BY USER  ".black().on_yellow().bold()
        );
        println!("{}", "═".repeat(50).yellow());
        println!();
        println!("{}", "Cleanup was stopped safely.".yellow());
        println!(
            "{}",
            format!(
                "Processed: {}/{} files before cancellation",
                stats.deleted_files, stats.total_files
            )
            .dimmed()
        );
        println!();
    }

    if dry_run {
        println!(
            "{} {}",
            "Would delete:".white(),
            format!("{} files", stats.deleted_files).yellow().bold()
        );
        println!(
            "{} {}",
            "Space to recover:".white(),
            format_size(stats.deleted_size).yellow().bold()
        );
    } else {
        println!(
            "{} {}",
            "Deleted:".green().bold(),
            format!("{} files", stats.deleted_files).yellow().bold()
        );
        println!(
            "{} {}",
            "Space recovered:".green().bold(),
            format_size(stats.deleted_size).yellow().bold()
        );

        if stats.failed_files > 0 {
            println!(
                "{} {} (files in use or protected)",
                "Failed:".red().bold(),
                format!("{} files", stats.failed_files).red()
            );
        }
    }

    // Show error statistics if any errors occurred
    if stats.permission_errors > 0 || stats.other_errors > 0 || stats.skipped_files > 0 {
        println!();
        println!("{}", "Details:".cyan().bold());

        if stats.skipped_files > 0 {
            // Determine the reason for skipping
            let reason = if min_age_hours.is_some() || !include_recent {
                "(too recent - younger than age filter)".to_string()
            } else {
                "(errors or other reasons)".to_string()
            };

            println!(
                "  {} {} files {}",
                "Skipped:".cyan(),
                stats.skipped_files,
                reason.dimmed()
            );
        }

        if stats.permission_errors > 0 {
            println!(
                "  {} {} files/dirs (may require administrator privileges)",
                "Permission denied:".yellow(),
                stats.permission_errors
            );
        }

        if stats.other_errors > 0 {
            println!(
                "  {} {} items",
                "Other errors:".yellow(),
                stats.other_errors
            );
        }
    }

    // Show inaccessible directories if any
    if !stats.inaccessible_dirs.is_empty() {
        println!();
        println!(
            "{}",
            "⚠️  Warning: Some directories could not be accessed:"
                .yellow()
                .bold()
        );
        for dir in &stats.inaccessible_dirs {
            let cleaned_path = dir.strip_prefix("\\\\?\\").unwrap_or(dir);
            println!("  • {}", cleaned_path.dimmed());
        }
        println!();
        println!(
            "{}",
            "These paths may no longer exist or are not accessible.".dimmed()
        );
        println!(
            "{}",
            "Use 'msc clean list' to review and 'msc clean remove' to remove invalid paths."
                .dimmed()
        );
    }
}

/// Handle 'clean start' command - Execute cleanup of temporary files
///
/// This implements the Principle of Least Privilege by separating cleanup into two phases:
/// 1. User directories: Cleaned WITHOUT admin privileges
/// 2. System directories: Cleaned WITH admin privileges (after user confirmation)
pub fn handle_start(matches: &clap::ArgMatches) -> Result<()> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let dry_run = matches.get_flag("dry-run");
    let min_age_hours = matches.get_one::<u64>("min-age");
    let include_recent = matches.get_flag("include-recent");
    let include_recycle = matches.get_flag("include-recycle");
    let work_cache = matches.get_flag("work-cache");

    // Create shared cancellation flag
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_flag_clone = cancel_flag.clone();

    // Setup Ctrl+C handler
    ctrlc::set_handler(move || {
        println!();
        println!("{}", "Cancellation requested...".yellow().bold());
        println!(
            "{}",
            "Stopping cleanup safely (this may take a moment)...".dimmed()
        );
        cancel_flag_clone.store(true, Ordering::Relaxed);
    })
    .map_err(|e| anyhow::anyhow!("Failed to set Ctrl+C handler: {}", e))?;

    if dry_run {
        println!(
            "{}",
            "DRY RUN MODE - No files will be deleted".yellow().bold()
        );
        println!();
    }

    println!("{}", "Starting cleanup of temporary files...".cyan().bold());
    println!("{}", "Press Ctrl+C at any time to cancel safely".dimmed());
    println!();

    // Determine age filters
    let (min_age, max_age) = if let Some(&hours) = min_age_hours {
        (Some(std::time::Duration::from_secs(hours * 3600)), None)
    } else if include_recent {
        (None, None)
    } else {
        (Some(std::time::Duration::from_secs(8 * 3600)), None) // Default: 8 hours
    };

    // Display age policy
    if include_recent {
        println!(
            "{}",
            "⚠️  Warning: Deleting files of ALL ages (including recent files)"
                .yellow()
                .bold()
        );
        println!(
            "{}",
            "This may delete temporary files from running applications!".yellow()
        );
    } else if let Some(hours) = min_age_hours {
        println!(
            "{}",
            format!("Only deleting files older than {} hours", hours).cyan()
        );
    } else {
        println!(
            "{}",
            "Only deleting files older than 24 hours (default)".cyan()
        );
        println!(
            "{}",
            "Use --min-age <HOURS> to customize or --include-recent to delete all".dimmed()
        );
    }
    println!();

    // Get all directories from config
    let config = Config::load()?;
    let mut all_directories = config.get_clean_paths();

    // Add Recycle Bin if requested
    if include_recycle {
        if let Some(recycle_path) = get_recycle_bin_directory() {
            println!(
                "{}",
                format!("Including Recycle Bin: {}", recycle_path).cyan()
            );
            all_directories.push(recycle_path);
            println!();
        } else {
            println!(
                "{}",
                "Warning: Could not find Recycle Bin directory.".yellow()
            );
            println!();
        }
    }

    if all_directories.is_empty() {
        println!("{}", "No temp directories found.".yellow());
        return Ok(());
    }

    // Categorize directories by privilege requirements
    let categorized = categorize_by_privilege(&all_directories);

    println!(
        "{}",
        "Directories categorized by privilege level:".white().bold()
    );
    println!();

    if !categorized.user_directories.is_empty() {
        println!("{}", "User directories (no admin required):".green());
        for (idx, dir) in categorized.user_directories.iter().enumerate() {
            println!("  {}. {}", idx + 1, dir.cyan());
        }
        println!();
    }

    if !categorized.system_directories.is_empty() {
        println!("{}", "System directories (admin required):".yellow());
        for (idx, dir) in categorized.system_directories.iter().enumerate() {
            println!("  {}. {}", idx + 1, dir.cyan());
        }
        println!();
    }

    // Ask for confirmation unless it's a dry run
    if !dry_run {
        println!(
            "{}",
            "⚠️  Warning: This will delete files in the directories listed above."
                .yellow()
                .bold()
        );
        println!();

        // Use robust confirmation with retry logic
        match read_confirmation("Do you want to continue? (y/n): ", 3) {
            Ok(true) => {
                // User confirmed, continue
            }
            Ok(false) => {
                println!();
                println!("{}", "Operation cancelled by user.".yellow());
                return Ok(());
            }
            Err(e) => {
                println!();
                println!("{}", format!("Error: {}", e).red());
                println!("{}", "Operation cancelled for safety.".yellow());
                return Err(e);
            }
        }

        println!();
    }

    // PHASE 1: Clean user directories WITHOUT admin privileges
    if !categorized.user_directories.is_empty() {
        println!("{}", "═".repeat(50).green());
        println!("{}", "PHASE 1: Cleaning User Directories".green().bold());
        println!(
            "{}",
            "(No administrator privileges needed)".green().dimmed()
        );
        println!("{}", "═".repeat(50).green());
        println!();

        let _user_stats = perform_cleanup(
            categorized.user_directories.clone(),
            dry_run,
            min_age,
            max_age,
            min_age_hours,
            include_recent,
            Some(cancel_flag.clone()),
        )?;

        println!();

        // Check if cancelled during user phase
        if cancel_flag.load(Ordering::Relaxed) {
            return Ok(());
        }
    }

    // PHASE 2: Clean system directories WITH admin privileges (after confirmation)
    if !categorized.system_directories.is_empty() {
        println!("{}", "═".repeat(50).yellow());
        println!("{}", "PHASE 2: Cleaning System Directories".yellow().bold());
        println!(
            "{}",
            "(Administrator privileges required)".yellow().dimmed()
        );
        println!("{}", "═".repeat(50).yellow());
        println!();

        // Check if already elevated
        #[cfg(windows)]
        let is_admin = is_elevated();
        #[cfg(not(windows))]
        let is_admin = false;

        if !is_admin {
            // Not elevated - ask for confirmation before requesting admin
            if !dry_run {
                println!(
                    "{}",
                    "System directories require administrator privileges.".yellow()
                );
                println!();

                match read_confirmation(
                    "Do you want to clean system directories? This will request admin access. (y/n): ",
                    3,
                ) {
                    Ok(true) => {
                        // User wants to clean system directories
                    }
                    Ok(false) => {
                        println!();
                        println!(
                            "{}",
                            "Skipping system directories cleanup.".yellow()
                        );
                        println!();
                        println!("{}", "User directories have been cleaned successfully.".green());
                        return Ok(());
                    }
                    Err(e) => {
                        println!();
                        println!("{}", format!("Error: {}", e).red());
                        println!("{}", "Skipping system directories for safety.".yellow());
                        println!();
                        println!("{}", "User directories have been cleaned successfully.".green());
                        return Ok(());
                    }
                }

                println!();

                // Request elevation
                #[cfg(windows)]
                {
                    println!("{}", "Requesting administrator privileges...".cyan());
                    if elevate_and_rerun()? {
                        println!("{}", "Relaunching with administrator privileges...".green());
                        return Ok(());
                    } else {
                        println!(
                            "{}",
                            "Warning: Could not elevate privileges. Skipping system directories."
                                .yellow()
                        );
                        println!();
                        println!(
                            "{}",
                            "User directories have been cleaned successfully.".green()
                        );
                        return Ok(());
                    }
                }

                #[cfg(not(windows))]
                {
                    println!(
                        "{}",
                        "Privilege elevation is only supported on Windows.".yellow()
                    );
                    println!("{}", "Try running this command with sudo/root.".yellow());
                    println!();
                    println!(
                        "{}",
                        "User directories have been cleaned successfully.".green()
                    );
                    return Ok(());
                }
            }
        }

        // If we're here, we have admin privileges (or it's a dry run)
        let _system_stats = perform_cleanup(
            categorized.system_directories.clone(),
            dry_run,
            min_age,
            max_age,
            min_age_hours,
            include_recent,
            Some(cancel_flag.clone()),
        )?;

        println!();
    }

    if !categorized.user_directories.is_empty() || !categorized.system_directories.is_empty() {
        println!("{}", "═".repeat(50).green());
        println!("{}", "✓ Cleanup completed successfully".green().bold());
        println!("{}", "═".repeat(50).green());
        println!();
    }

    // WORK CACHE CLEANUP: Clean cache folders in work directory projects if requested
    if work_cache {
        clean_work_cache(&config, dry_run)?;
    }

    Ok(())
}

/// Handle 'clean add' command - Add a custom clean path
pub fn handle_add(matches: &clap::ArgMatches) -> Result<()> {
    use crate::core::{PathValidator, ValidationResult};

    let path = matches
        .get_one::<String>("path")
        .ok_or_else(|| anyhow::anyhow!("Path is required"))?;
    let force = matches.get_flag("force");

    let path_buf = Path::new(path);

    // Validate path using PathValidator
    let validator = PathValidator::new();
    let validation = validator.validate_path(path_buf)?;

    let canonical_path = match validation {
        ValidationResult::Forbidden(msg) => {
            println!("{}", "╔═══════════════════════════════════════════╗".red());
            println!(
                "{}",
                "║  FORBIDDEN - CANNOT ADD THIS PATH        ║".red().bold()
            );
            println!("{}", "╚═══════════════════════════════════════════╝".red());
            println!();
            println!("{}", msg.red());
            println!();
            println!("{}", "This is a protected system directory.".red());
            println!("{}", "Adding it could destroy your system.".red());
            return Ok(());
        }

        ValidationResult::Error(msg) => {
            println!("{}", format!("Error: {}", msg).red());
            return Ok(());
        }

        ValidationResult::Warning(msg, path) => {
            println!("{}", "⚠️  WARNING".yellow().bold());
            println!("{}", msg.yellow());
            println!();

            if !force {
                match read_exact_confirmation(
                    "Are you ABSOLUTELY sure? Type 'YES' to confirm: ",
                    "YES",
                ) {
                    Ok(true) => {
                        // User confirmed correctly
                    }
                    Ok(false) => {
                        println!();
                        println!("{}", "Operation cancelled.".yellow());
                        return Ok(());
                    }
                    Err(e) => {
                        // Error already printed by read_exact_confirmation
                        return Err(e);
                    }
                }
            }

            path
        }

        ValidationResult::Safe(path) => path,
    };

    let canonical_str = canonical_path.to_string_lossy().to_string();

    // Load config
    let mut config = Config::load()?;

    // Add path
    if config.add_clean_path(canonical_str.clone()) {
        config.save()?;
        let cleaned_path = canonical_str
            .strip_prefix("\\\\?\\")
            .unwrap_or(&canonical_str);
        println!();
        println!("{}", "✓ Clean path added successfully:".green());
        println!("  {}", cleaned_path.cyan());
        println!();
        println!(
            "{}",
            "This path will be included in future cleanup operations.".dimmed()
        );
    } else {
        println!("{}", "Path already exists in clean paths.".yellow());
    }

    Ok(())
}

/// Handle 'clean list' command - List all active clean paths
pub fn handle_list(_matches: &clap::ArgMatches) -> Result<()> {
    // Load config - this automatically syncs default paths
    let config = Config::load()?;
    let active_paths = config.get_clean_paths();

    println!("{}", "═".repeat(70).white());
    println!("{}", "  ACTIVE CLEAN PATHS  ".white().bold());
    println!("{}", "═".repeat(70).white());
    println!();

    if active_paths.is_empty() {
        println!("  {}", "No clean paths configured.".dimmed());
        println!();
        println!("{}", "To add a custom path, run:".dimmed());
        println!("  {}", "msc clean add <path>".cyan());
    } else {
        println!("{}", "Default & Custom Paths:".white().bold());
        for (index, dir) in active_paths.iter().enumerate() {
            let cleaned_path = dir.strip_prefix("\\\\?\\").unwrap_or(dir);
            println!(
                "  {}. {}",
                (index + 1).to_string().dimmed(),
                cleaned_path.cyan()
            );
        }
    }

    println!();
    println!("{}", "─".repeat(70).dimmed());
    println!();

    // Show Special Paths (Recycle Bin and Work Cache)
    println!("{}", "Special Paths:".white().bold());
    println!();

    // Recycle Bin
    println!("{}", "  Recycle Bin:".cyan());
    if let Some(recycle_path) = get_recycle_bin_directory() {
        println!("    {} {}", "•".dimmed(), recycle_path.yellow());
        println!(
            "      {}",
            "(only included with --include-recycle or --IR flag)".dimmed()
        );
    } else {
        println!(
            "    {} {}",
            "•".dimmed(),
            "Not available on this system".dimmed()
        );
    }

    println!();

    // Work Cache Paths
    println!("{}", "  Work Directory Cache:".cyan());
    if let Some(work_path) = config.get_work_path() {
        // Clean the Windows long path prefix
        let cleaned_work_path = work_path.strip_prefix("\\\\?\\").unwrap_or(work_path);
        let cache_folders = ["target", "dist", "node_modules"];

        for cache_folder in &cache_folders {
            println!(
                "    {} {}",
                "•".dimmed(),
                format!("{}\\<project>\\{}", cleaned_work_path, cache_folder).yellow()
            );
        }

        println!();
        println!(
            "      {}",
            "(only included with --work-cache or -WC flag)".dimmed()
        );

        // Show ignored folders
        let ignored_folders = config.get_ignored_work_folders();
        if !ignored_folders.is_empty() {
            println!();
            println!("      {}", "Ignored project folders:".dimmed());
            for folder in &ignored_folders {
                println!("        {} {}", "↳".dimmed(), folder.dimmed());
            }
        }
    } else {
        println!(
            "    {} {}",
            "•".dimmed(),
            "No work directory configured".dimmed()
        );
        println!(
            "      {}",
            "Use 'msc set work <path>' to configure a work directory".dimmed()
        );
    }

    println!();
    println!("{}", "═".repeat(70).white());
    println!();

    Ok(())
}

/// Handle 'clean remove' command - Remove a clean path interactively
pub fn handle_remove(_matches: &clap::ArgMatches) -> Result<()> {
    // Load config - this automatically syncs default paths
    let mut config = Config::load()?;
    let active_paths = config.get_clean_paths();

    if active_paths.is_empty() {
        println!("{}", "No clean paths configured to remove.".yellow());
        return Ok(());
    }

    // Show interactive selection
    let selection_result = select_from_list("Select a path to remove:", &active_paths);

    match selection_result {
        Err(e) => {
            println!();
            println!("{}", format!("Error: {}", e).red().bold());
            println!();
            println!(
                "{}",
                "Tip: Make sure you're running in a compatible terminal (CMD, PowerShell, or Windows Terminal)".dimmed()
            );
            return Err(anyhow::anyhow!("Selection failed: {}", e));
        }
        Ok(None) => {
            println!();
            println!("{}", "Operation cancelled.".yellow());
            return Ok(());
        }
        Ok(Some(selected_index)) => {
            let selected_path = &active_paths[selected_index];
            let cleaned_path = selected_path
                .strip_prefix("\\\\?\\")
                .unwrap_or(selected_path);

            println!();
            println!("{}", "Removing path...".dimmed());
            println!();

            // Simply remove the path from the list
            if config.remove_clean_path(selected_path) {
                config.save()?;
                println!("{}", "✓ Path removed successfully:".green().bold());
                println!("  {}", cleaned_path.cyan());
                println!();
                println!(
                    "{}",
                    "This path will no longer be used for cleaning.".dimmed()
                );
            } else {
                println!("{}", "✗ Failed to remove path.".red().bold());
                println!("  {}", cleaned_path.cyan());
            }
            println!();
        }
    }

    Ok(())
}

/// Handle 'clean reset' command - Reset to default configuration
pub fn handle_clear(_matches: &clap::ArgMatches) -> Result<()> {
    // Load config - this automatically syncs default paths
    let mut config = Config::load()?;
    let current_count = config.get_clean_paths().len();

    // Check if already at defaults (no custom paths and no exclusions)
    let is_at_defaults =
        config.get_custom_paths().is_empty() && config.excluded_default_paths.is_empty();

    if is_at_defaults {
        println!("{}", "Already using default configuration.".yellow());
        println!();
        println!("{}", "All paths are set to system defaults.".dimmed());
        return Ok(());
    }

    println!(
        "{}",
        "Resetting clean paths to default configuration..."
            .cyan()
            .bold()
    );
    println!();

    // Reset to defaults
    config.reset_to_defaults();
    config.save()?;

    let new_count = config.get_clean_paths().len();

    println!("{}", "✓ Configuration reset successfully".green().bold());
    println!();
    println!(
        "{}",
        format!("Active paths: {} → {}", current_count, new_count).dimmed()
    );
    println!("{}", "All default system paths are now active.".dimmed());
    println!();

    Ok(())
}

/// Handle 'clean ignore add' command - Add a folder to the ignore list
pub fn handle_ignore_add(matches: &clap::ArgMatches) -> Result<()> {
    let folder = matches
        .get_one::<String>("folder")
        .ok_or_else(|| anyhow::anyhow!("Folder is required"))?;

    // Load config
    let mut config = Config::load()?;

    // Check if it's "msc" (already always ignored)
    if folder == "msc" {
        println!(
            "{}",
            "The 'msc' folder is always ignored automatically.".yellow()
        );
        println!("{}", "You don't need to add it manually.".dimmed());
        return Ok(());
    }

    // Try to add the folder
    if config.add_ignored_work_folder(folder.clone()) {
        config.save()?;
        println!();
        println!("{}", "✓ Folder added to ignore list:".green().bold());
        println!("  {}", folder.cyan());
        println!();
        println!(
            "{}",
            "This folder will be skipped during work cache cleanup.".dimmed()
        );
        println!();
    } else {
        println!("{}", "Folder already in ignore list.".yellow());
        println!("  {}", folder.cyan());
    }

    Ok(())
}

/// Handle 'clean ignore list' command - List all ignored folders
pub fn handle_ignore_list(_matches: &clap::ArgMatches) -> Result<()> {
    let config = Config::load()?;
    let ignored_folders = config.get_ignored_work_folders();

    println!("{}", "═".repeat(50).white());
    println!("{}", "  IGNORED WORK FOLDERS  ".white().bold());
    println!("{}", "═".repeat(50).white());
    println!();

    if ignored_folders.is_empty() {
        println!("  {}", "No folders configured to ignore.".dimmed());
        println!();
        println!(
            "{}",
            "Note: 'msc' is always ignored automatically.".dimmed()
        );
    } else {
        println!(
            "{}",
            "Folders that will be skipped during work cache cleanup:".white()
        );
        println!();

        for (index, folder) in ignored_folders.iter().enumerate() {
            if folder == "msc" {
                println!(
                    "  {}. {} {}",
                    (index + 1).to_string().dimmed(),
                    folder.cyan(),
                    "(automatic)".dimmed()
                );
            } else {
                println!("  {}. {}", (index + 1).to_string().dimmed(), folder.cyan());
            }
        }
    }

    println!();
    println!("{}", "═".repeat(50).white());
    println!();

    Ok(())
}

/// Handle 'clean ignore remove' command - Remove a folder from the ignore list
pub fn handle_ignore_remove(matches: &clap::ArgMatches) -> Result<()> {
    let folder = matches
        .get_one::<String>("folder")
        .ok_or_else(|| anyhow::anyhow!("Folder is required"))?;

    // Load config
    let mut config = Config::load()?;

    // Check if it's "msc" (always ignored)
    if folder == "msc" {
        println!("{}", "Cannot remove 'msc' from ignore list.".yellow());
        println!(
            "{}",
            "This folder is always ignored automatically for safety.".dimmed()
        );
        return Ok(());
    }

    // Try to remove the folder
    if config.remove_ignored_work_folder(folder) {
        config.save()?;
        println!();
        println!("{}", "✓ Folder removed from ignore list:".green().bold());
        println!("  {}", folder.cyan());
        println!();
        println!(
            "{}",
            "This folder will now be included in work cache cleanup.".dimmed()
        );
        println!();
    } else {
        println!("{}", "Folder not found in ignore list.".yellow());
        println!("  {}", folder.cyan());
        println!();
        println!(
            "{}",
            "Use 'msc clean ignore list' to see all ignored folders.".dimmed()
        );
    }

    Ok(())
}
