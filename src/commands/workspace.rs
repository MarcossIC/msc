use crate::core::WorkspaceManager;
use anyhow::Result;
use colored::Colorize;

pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("map", _)) => map_workspaces(),
        Some(("list", _)) => list_workspaces(),
        _ => {
            println!("Use 'msc work --help' for more information.");
            Ok(())
        }
    }
}

fn map_workspaces() -> Result<()> {
    let mut manager = WorkspaceManager::new()?;

    // Check if work path is configured
    if manager.config().get_work_path().is_none() {
        println!("{}", "No work directory configured.".yellow());
        println!();
        println!("{}", "To set a work directory first, run:".white());
        println!("  {}", "msc set work <path>".cyan().bold());
        println!();
        println!("{}", "Example:".dimmed());
        println!("  {}", "msc set work C:\\Users\\marco\\projects".dimmed());
        return Ok(());
    }

    println!("{}", "Mapping workspaces...".cyan());
    println!();

    match manager.map_workspaces() {
        Ok(count) => {
            println!();
            println!(
                "{} {}",
                "Successfully mapped".green().bold(),
                format!("{} workspace(s)", count).yellow().bold()
            );
            Ok(())
        }
        Err(e) => {
            println!("{}", format!("Error: {}", e).red());
            Ok(())
        }
    }
}

fn list_workspaces() -> Result<()> {
    let manager = WorkspaceManager::new()?;
    let workspaces = manager.list_workspaces();

    if workspaces.is_empty() {
        println!(
            "{}",
            "No workspaces found. Use 'msc work map' to map your project folders.".yellow()
        );
        return Ok(());
    }

    println!(
        "{} {}",
        "Workspaces:".white().bold(),
        format!("({} total)", workspaces.len()).dimmed()
    );
    println!();

    for (name, path) in workspaces {
        let cleaned_path = path.strip_prefix("\\\\?\\").unwrap_or(&path);
        println!(
            "  {} {}",
            "📂".to_string().blue().bold(),
            name.cyan().bold()
        );
        println!("     {}", cleaned_path.dimmed());
    }

    Ok(())
}
