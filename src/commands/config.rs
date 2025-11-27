use crate::core::{validation, Config};
use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;

/// Configuration type enum for DRY code
enum ConfigType {
    Work,
    Video,
}

impl ConfigType {
    fn name(&self) -> &'static str {
        match self {
            ConfigType::Work => "Work",
            ConfigType::Video => "Video",
        }
    }

    fn set_path(&self, config: &mut Config, path: String) {
        match self {
            ConfigType::Work => config.set_work_path(path),
            ConfigType::Video => config.set_video_path(path),
        }
    }

    fn get_path<'a>(&self, config: &'a Config) -> Option<&'a String> {
        match self {
            ConfigType::Work => config.get_work_path(),
            ConfigType::Video => config.get_video_path(),
        }
    }

    fn example_path(&self) -> &'static str {
        match self {
            ConfigType::Work => "C:\\Users\\marco\\projects",
            ConfigType::Video => "C:\\Users\\marco\\videos",
        }
    }
}

pub fn handle_set(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("work", sub_matches)) => set_path_for_type(sub_matches, ConfigType::Work),
        Some(("video", sub_matches)) => set_path_for_type(sub_matches, ConfigType::Video),
        _ => {
            println!("Use 'msc set --help' for more information.");
            Ok(())
        }
    }
}

/// Shared logic for setting a path configuration
fn set_path_for_type(matches: &clap::ArgMatches, config_type: ConfigType) -> Result<()> {
    let path = matches
        .get_one::<String>("path")
        .context("Path argument is required")?;

    // Validar el path antes de usarlo
    validation::validate_directory_path(path)?;

    let path_buf = Path::new(path);
    if !path_buf.exists() {
        println!(
            "{}",
            format!("⚠️  Warning: Path '{}' does not exist", path).yellow()
        );
        println!(
            "{}",
            "The path will be saved but may not be usable until created.".dimmed()
        );
    }

    let canonical_path = if path_buf.exists() {
        path_buf
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("Failed to resolve path: {}", e))?
            .to_string_lossy()
            .to_string()
    } else {
        path.to_string()
    };

    let mut config = Config::load()?;
    config_type.set_path(&mut config, canonical_path.clone());
    config.save()?;

    println!(
        "{} {}",
        format!("✓ {} path set to:", config_type.name()).green(),
        canonical_path
    );

    Ok(())
}

pub fn handle_get(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("work", _)) => get_path_for_type(ConfigType::Work),
        Some(("video", _)) => get_path_for_type(ConfigType::Video),
        _ => {
            println!("Use 'msc get --help' for more information.");
            Ok(())
        }
    }
}

/// Shared logic for getting a path configuration
fn get_path_for_type(config_type: ConfigType) -> Result<()> {
    let config = Config::load()?;
    let type_name = config_type.name();
    let type_lower = type_name.to_lowercase();

    match config_type.get_path(&config) {
        Some(path) => {
            let cleaned_path = path.strip_prefix("\\\\?\\").unwrap_or(path);
            println!("{}", format!("{} directory path:", type_name).white());
            println!("{}", cleaned_path.cyan().bold());
        }
        None => {
            println!(
                "{}",
                format!("No {} directory configured.", type_lower).yellow()
            );
            println!();
            println!(
                "{}",
                format!("To set a {} directory, run:", type_lower).white()
            );
            println!(
                "  {}",
                format!("msc set {} <path>", type_lower).cyan().bold()
            );
            println!();
            println!("{}", "Example:".dimmed());
            println!(
                "  {}",
                format!("msc set {} {}", type_lower, config_type.example_path()).dimmed()
            );
        }
    }

    Ok(())
}
