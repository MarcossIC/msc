use anyhow::Result;
use colored::Colorize;
use crate::core::TempCleaner;
use crate::ui::{format_size};
use crate::platform::{is_elevated, elevate_and_rerun};

pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    let dry_run = matches.get_flag("dry-run");

    if dry_run {
        println!("{}", "DRY RUN MODE - No files will be deleted".yellow().bold());
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
        println!("{}", "⚠️  Warning: This will delete all files in the directories listed above.".yellow().bold());

        #[cfg(windows)]
        {
            if !is_elevated() {
                println!("{}", "Note: Administrator privileges are required for system directories.".yellow());
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
                    // Successfully relaunched with admin privileges, exit this instance
                    println!("{}", "Relaunching with administrator privileges...".green());
                    return Ok(());
                } else {
                    println!("{}", "Warning: Could not elevate privileges. Some files may fail to delete.".yellow());
                    println!();
                }
            }
        }
    }

    // Scan files
    println!("{}", "Scanning temporary files...".dimmed());
    let scan_stats = cleaner.scan();

    if scan_stats.total_files == 0 {
        println!("{}", "No temporary files found to clean.".green());
        return Ok(());
    }

    println!("{} {} files ({}) found",
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

        print!("\r{} [{}{}] {}% ({}/{}) ",
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
        println!("{} {}", "Would delete:".white(), format!("{} files", stats.deleted_files).yellow().bold());
        println!("{} {}", "Space to recover:".white(), format_size(stats.deleted_size).yellow().bold());
    } else {
        println!("{} {}", "Deleted:".green().bold(), format!("{} files", stats.deleted_files).yellow().bold());
        println!("{} {}", "Space recovered:".green().bold(), format_size(stats.deleted_size).yellow().bold());

        if stats.failed_files > 0 {
            println!("{} {} (files in use or protected)",
                "Failed:".red().bold(),
                format!("{} files", stats.failed_files).red()
            );
        }
    }

    println!();

    Ok(())
}
