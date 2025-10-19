use crate::core::Config;
use anyhow::Result;
use colored::Colorize;
use std::path::Path;

pub fn handle_set(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("work", sub_matches)) => {
            let path = sub_matches.get_one::<String>("path").unwrap();

            let path_buf = Path::new(path);
            if !path_buf.exists() {
                println!("Warning: Path '{}' does not exist", path);
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
            config.set_work_path(canonical_path.clone());
            config.save()?;

            println!("Work path set to: {}", canonical_path);
        }
        _ => {
            println!("Use 'msc set --help' for more information.");
        }
    }

    Ok(())
}

pub fn handle_get(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("work", _)) => {
            let config = Config::load()?;

            match config.get_work_path() {
                Some(path) => {
                    let cleaned_path = path.strip_prefix("\\\\?\\").unwrap_or(path);
                    println!("{}", "Work directory path:".white());
                    println!("{}", cleaned_path.cyan().bold());
                }
                None => {
                    println!("{}", "No work directory configured.".yellow());
                    println!();
                    println!("{}", "To set a work directory, run:".white());
                    println!("  {}", "msc set work <path>".cyan().bold());
                    println!();
                    println!("{}", "Example:".dimmed());
                    println!("  {}", "msc set work C:\\Users\\marco\\projects".dimmed());
                }
            }
        }
        _ => {
            println!("Use 'msc get --help' for more information.");
        }
    }

    Ok(())
}
