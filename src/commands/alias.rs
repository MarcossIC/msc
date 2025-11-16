use anyhow::{Context, Result};
use clap::ArgMatches;
use colored::Colorize;

use crate::core::{get_generator, Alias, AliasConfig, PathManager};

/// Main handler for alias commands
pub fn handle_alias(matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("add", sub_matches)) => handle_add(sub_matches),
        Some(("remove", sub_matches)) => handle_remove(sub_matches),
        Some(("list", sub_matches)) => handle_list(sub_matches),
        Some(("init", sub_matches)) => handle_init(sub_matches),
        Some(("nuke", sub_matches)) => handle_nuke(sub_matches),
        _ => {
            println!("Use 'msc alias --help' for more information.");
            Ok(())
        }
    }
}

/// Handle alias add command
fn handle_add(matches: &ArgMatches) -> Result<()> {
    let name = matches
        .get_one::<String>("name")
        .context("Alias name is required")?;
    let command = matches
        .get_one::<String>("command")
        .context("Command is required")?;
    let description = matches.get_one::<String>("description");

    // Validate alias name
    if !is_valid_alias_name(name) {
        anyhow::bail!(
            "Invalid alias name '{}'. Only alphanumeric characters, hyphens, and underscores are allowed.",
            name
        );
    }

    // Load config
    let mut config = AliasConfig::load().context("Failed to load alias configuration")?;

    // Check if alias already exists
    let is_new = !config.exists(name);
    if !is_new {
        println!(
            "{}",
            format!("⚠️  Alias '{}' already exists. It will be updated.", name).yellow()
        );
    }

    // Create alias
    let alias = if let Some(desc) = description {
        Alias::with_description(name.clone(), command.clone(), desc.clone())
    } else {
        Alias::new(name.clone(), command.clone())
    };

    // Add to config
    config.add_alias(alias.clone());

    // Save config
    config
        .save()
        .context("Failed to save alias configuration")?;

    // Generate executable
    let bin_dir = PathManager::ensure_bin_dir().context("Failed to ensure bin directory exists")?;

    let generator =
        get_generator().context("Failed to get alias generator for current platform")?;

    generator
        .generate(&alias, &bin_dir)
        .context("Failed to generate alias executable")?;

    // Success message
    if is_new {
        println!(
            "{}",
            format!("✓ Alias '{}' created successfully!", name).green()
        );
    } else {
        println!(
            "{}",
            format!("✓ Alias '{}' updated successfully!", name).green()
        );
    }
    println!("  Command: {}", command.cyan());

    // Check if bin dir is in PATH
    if !PathManager::is_in_path().unwrap_or(false) {
        println!();
        println!(
            "{}",
            "⚠️  The aliases directory is not in your PATH.".yellow()
        );
        println!(
            "   Run {} to add it to your PATH.",
            "'msc alias init'".cyan()
        );
    }

    Ok(())
}

/// Handle alias remove command
fn handle_remove(matches: &ArgMatches) -> Result<()> {
    let name = matches
        .get_one::<String>("name")
        .context("Alias name is required")?;

    // Load config
    let mut config = AliasConfig::load().context("Failed to load alias configuration")?;

    // Check if alias exists
    if !config.exists(name) {
        anyhow::bail!("Alias '{}' does not exist.", name);
    }

    // Remove from config
    config.remove_alias(name);

    // Save config
    config
        .save()
        .context("Failed to save alias configuration")?;

    // Clean up executable
    let bin_dir = PathManager::get_aliases_bin_dir().context("Failed to get bin directory")?;

    let generator =
        get_generator().context("Failed to get alias generator for current platform")?;

    generator
        .cleanup(name, &bin_dir)
        .context("Failed to remove alias executable")?;

    // Success message
    println!(
        "{}",
        format!("✓ Alias '{}' removed successfully!", name).green()
    );

    Ok(())
}

/// Handle alias list command
fn handle_list(_matches: &ArgMatches) -> Result<()> {
    // Load config
    let config = AliasConfig::load().context("Failed to load alias configuration")?;

    let aliases = config.list_aliases();

    if aliases.is_empty() {
        println!("No aliases configured.");
        println!(
            "Use {} to create a new alias.",
            "'msc alias add <name> <command>'".cyan()
        );
        return Ok(());
    }

    println!("{}", "Configured Aliases:".bold());
    println!();

    for alias in aliases {
        println!("  {} {}", "•".cyan(), alias.name.bold());
        println!("    Command: {}", alias.command.cyan());
        if let Some(desc) = &alias.description {
            println!("    Description: {}", desc);
        }
        println!();
    }

    // Show PATH status
    let in_path = PathManager::is_in_path().unwrap_or(false);
    if !in_path {
        println!(
            "{}",
            "⚠️  The aliases directory is not in your PATH.".yellow()
        );
        println!(
            "   Run {} to add it to your PATH.",
            "'msc alias init'".cyan()
        );
    } else {
        println!("{}", "✓ Aliases directory is in your PATH.".green());
    }

    Ok(())
}

/// Handle alias init command (setup PATH)
fn handle_init(_matches: &ArgMatches) -> Result<()> {
    println!("{}", "Initializing MSC alias system...".bold());

    // Ensure bin directory exists
    let bin_dir = PathManager::ensure_bin_dir().context("Failed to create bin directory")?;

    println!("  Bin directory: {}", bin_dir.display().to_string().cyan());

    // Check if already in PATH
    if PathManager::is_in_path()? {
        println!("{}", "✓ Aliases directory is already in your PATH.".green());
        return Ok(());
    }

    // Add to PATH
    println!("  Adding to PATH...");
    PathManager::add_to_path().context("Failed to add bin directory to PATH")?;

    #[cfg(windows)]
    {
        println!();
        println!("{}", "✓ Successfully added to PATH!".green());
        println!();
        println!(
            "{}",
            "⚠️  You may need to restart your terminal or computer for changes to take effect."
                .yellow()
        );
        println!("   Alternatively, you can run this command in a new terminal window to test.");
    }

    #[cfg(unix)]
    {
        println!();
        println!("{}", "✓ Successfully added to shell configuration!".green());
        println!();
        println!("   To apply changes immediately, run:");

        let shell = std::env::var("SHELL").unwrap_or_default();
        if shell.contains("zsh") {
            println!("   {}", "source ~/.zshrc".cyan());
        } else if shell.contains("fish") {
            println!("   {}", "source ~/.config/fish/config.fish".cyan());
        } else {
            println!("   {}", "source ~/.bashrc".cyan());
        }

        println!();
        println!("   Or restart your terminal.");
    }

    Ok(())
}

/// Handle alias nuke command (clean everything)
fn handle_nuke(_matches: &ArgMatches) -> Result<()> {
    println!(
        "{}",
        "⚠️  WARNING: This will completely remove all alias configuration!"
            .yellow()
            .bold()
    );
    println!();
    println!("This will:");
    println!("  • Remove all alias executables");
    println!("  • Delete the alias configuration file");
    println!("  • Remove the aliases directory from your PATH");
    println!("  • Delete the entire aliases directory");
    println!();

    // Ask for confirmation
    print!(
        "{}",
        "Are you sure you want to continue? (yes/no): ".yellow()
    );
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    if input != "yes" && input != "y" {
        println!("{}", "Nuke cancelled.".green());
        return Ok(());
    }

    println!();
    println!("{}", "Nuking alias system...".bold());

    // Step 1: Remove from PATH
    println!("  Removing from PATH...");
    match PathManager::remove_from_path() {
        Ok(_) => println!("    {}", "✓ Removed from PATH".green()),
        Err(e) => println!("    {} {}", "⚠️  Warning:".yellow(), e),
    }

    // Step 2: Get the aliases directory path
    let config_dir = dirs::config_dir().context("Could not determine config directory")?;
    let aliases_dir = config_dir.join("msc").join("aliases");

    // Step 3: Remove the entire aliases directory
    if aliases_dir.exists() {
        println!("  Removing aliases directory...");
        std::fs::remove_dir_all(&aliases_dir)
            .with_context(|| format!("Failed to remove directory: {:?}", aliases_dir))?;
        println!("    {}", "✓ Removed aliases directory".green());
    } else {
        println!("    {}", "✓ Aliases directory already removed".green());
    }

    println!();
    println!("{}", "✓ Alias system completely removed!".green().bold());
    println!();

    #[cfg(windows)]
    {
        println!(
            "{}",
            "Note: You may need to restart your terminal for PATH changes to take effect.".yellow()
        );
    }

    #[cfg(unix)]
    {
        println!(
            "{}",
            "Note: You may need to restart your terminal or run 'source ~/.bashrc' (or your shell's rc file).".yellow()
        );
    }

    Ok(())
}

/// Validate alias name (only alphanumeric, hyphens, and underscores)
fn is_valid_alias_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_alias_name() {
        assert!(is_valid_alias_name("pyh"));
        assert!(is_valid_alias_name("git-push"));
        assert!(is_valid_alias_name("my_alias"));
        assert!(is_valid_alias_name("alias123"));

        assert!(!is_valid_alias_name(""));
        assert!(!is_valid_alias_name("my alias"));
        assert!(!is_valid_alias_name("alias/test"));
        assert!(!is_valid_alias_name("alias\\test"));
        assert!(!is_valid_alias_name("alias$test"));
    }
}
