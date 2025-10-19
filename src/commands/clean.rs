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
use crate::platform::{elevate_and_rerun, is_elevated};
use crate::ui::{format_size, select_from_list};

/// Handle 'clean start' command - Execute cleanup of temporary files
pub fn handle_start(matches: &clap::ArgMatches) -> Result<()> {
    let dry_run = matches.get_flag("dry-run");

    if dry_run {
        println!(
            "{}",
            "DRY RUN MODE - No files will be deleted".yellow().bold()
        );
        println!();
    }

    println!("{}", "Starting cleanup of temporary files...".cyan().bold());
    println!();

    let cleaner = TempCleaner::new()?;

    if cleaner.directories.is_empty() {
        println!("{}", "No temp directories found.".yellow());
        return Ok(());
    }

    println!("{}", "Directories to clean:".white().bold());
    for (idx, dir) in cleaner.directories.iter().enumerate() {
        println!("  {}. {}", idx + 1, dir.cyan());
    }
    println!();

    // Ask for confirmation unless it's a dry run
    if !dry_run {
        println!(
            "{}",
            "⚠️  Warning: This will delete all files in the directories listed above."
                .yellow()
                .bold()
        );

        #[cfg(windows)]
        {
            if !is_elevated() {
                println!(
                    "{}",
                    "Note: Administrator privileges are required for system directories.".yellow()
                );
            }
        }

        println!();
        print!("{}", "Do you want to continue? (y/n): ".white().bold());

        use std::io::Write;
        std::io::stdout().flush().ok();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();

        let response = input.trim().to_lowercase();
        if response != "y" && response != "yes" {
            println!();
            println!("{}", "Operation cancelled by user.".yellow());
            return Ok(());
        }

        println!();

        // Check if we need elevation and attempt to elevate
        #[cfg(windows)]
        {
            if !is_elevated() {
                println!("{}", "Requesting administrator privileges...".cyan());
                if elevate_and_rerun()? {
                    println!("{}", "Relaunching with administrator privileges...".green());
                    return Ok(());
                } else {
                    println!(
                        "{}",
                        "Warning: Could not elevate privileges. Some files may fail to delete."
                            .yellow()
                    );
                    println!();
                }
            }
        }
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
        return Ok(());
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
    println!("{}", "─".repeat(50));
    println!("{}", "Cleanup Summary".white().bold());
    println!("{}", "─".repeat(50));

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

    println!();

    Ok(())
}

/// Handle 'clean add' command - Add a custom clean path
pub fn handle_add(matches: &clap::ArgMatches) -> Result<()> {
    let path = matches.get_one::<String>("path").expect("Path is required");

    let path_buf = Path::new(path);

    // Validate path exists
    if !path_buf.exists() {
        println!("{}", format!("Error: Path '{}' does not exist", path).red());
        return Ok(());
    }

    // Validate it's a directory
    if !path_buf.is_dir() {
        println!(
            "{}",
            format!("Error: Path '{}' is not a directory", path).red()
        );
        return Ok(());
    }

    // Canonicalize path
    let canonical_path = path_buf
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Failed to resolve path: {}", e))?
        .to_string_lossy()
        .to_string();

    // Load config
    let mut config = Config::load()?;

    // Add path
    if config.add_clean_path(canonical_path.clone()) {
        config.save()?;
        let cleaned_path = canonical_path
            .strip_prefix("\\\\?\\")
            .unwrap_or(&canonical_path);
        println!("{}", "Clean path added successfully:".green());
        println!("  {}", cleaned_path.cyan());
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

    println!("{}", "Active Clean Paths".white().bold());
    println!();

    if active_paths.is_empty() {
        println!("  {}", "No clean paths configured.".dimmed());
        println!();
        println!("{}", "To add a custom path, run:".dimmed());
        println!("  {}", "msc clean add <path>".cyan());
    } else {
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
