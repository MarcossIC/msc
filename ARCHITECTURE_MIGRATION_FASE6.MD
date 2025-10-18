### FASE 6: Extraer Commands (ALTO RIESGO - Hacer con cuidado)
**Duración estimada**: 4-6 horas
**Riesgo**: Alto

#### Paso 6.1: Crear commands/clean_temp.rs
```rust
// src/commands/clean_temp.rs
use anyhow::Result;
use colored::Colorize;
use crate::core::cleaner::TempCleaner;
use crate::ui::{prompts, formatters};
use crate::system::elevation;

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

    // Ask for confirmation
    if !dry_run {
        prompts::warn("This will delete all files in the directories listed above.");

        #[cfg(windows)]
        {
            if !elevation::is_elevated() {
                println!("{}", "Note: Administrator privileges are required for system directories.".yellow());
            }
        }

        println!();
        if !prompts::confirm("Do you want to continue?")? {
            println!();
            println!("{}", "Operation cancelled by user.".yellow());
            return Ok(());
        }

        println!();

        // Attempt elevation if needed
        #[cfg(windows)]
        {
            if !elevation::is_elevated() {
                prompts::info("Requesting administrator privileges...");
                if elevation::elevate_and_rerun()? {
                    prompts::success("Relaunching with administrator privileges...");
                    return Ok(());
                } else {
                    prompts::warn("Could not elevate privileges. Some files may fail to delete.");
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
        formatters::format_size(scan_stats.total_size).yellow().bold()
    );
    println!();

    if dry_run {
        println!("{}", "Files that would be deleted:".white().bold());
    } else {
        println!("{}", "Cleaning...".cyan().bold());
    }
    println!();

    // Clean with progress
    let stats = cleaner.clean(dry_run, |processed, total| {
        let percentage = (processed as f64 / total as f64 * 100.0) as usize;
        let bar_length = 30;
        let filled = (percentage as f64 / 100.0 * bar_length as f64) as usize;
        let empty = bar_length - filled;

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
        println!("{} {}", "Space to recover:".white(), formatters::format_size(stats.deleted_size).yellow().bold());
    } else {
        println!("{} {}", "Deleted:".green().bold(), format!("{} files", stats.deleted_files).yellow().bold());
        println!("{} {}", "Space recovered:".green().bold(), formatters::format_size(stats.deleted_size).yellow().bold());

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
```

#### Paso 6.2: Crear commands/workspace.rs
```rust
// src/commands/workspace.rs
use anyhow::Result;
use colored::Colorize;
use crate::core::workspace::WorkspaceManager;

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

    println!("{}", "Mapping workspaces...".cyan());
    println!();

    match manager.map_workspaces() {
        Ok(count) => {
            println!();
            println!("{} {}",
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
        println!("{}", "No workspaces found. Use 'msc work map' to map your project folders.".yellow());
        return Ok(());
    }

    println!("{} {}",
        "Workspaces:".white().bold(),
        format!("({} total)", workspaces.len()).dimmed()
    );
    println!();

    for (name, path) in workspaces {
        let cleaned_path = path.strip_prefix("\\\\?\\").unwrap_or(&path);
        println!("  {} {}", "📂".to_string().blue().bold(), name.cyan().bold());
        println!("     {}", cleaned_path.dimmed());
    }

    Ok(())
}
```

#### Paso 6.3: Crear commands/config.rs
```rust
// src/commands/config.rs
use anyhow::Result;
use std::path::Path;
use colored::Colorize;
use crate::core::config::Config;

pub fn handle_set(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("work", sub_matches)) => {
            let path = sub_matches.get_one::<String>("path").unwrap();

            let path_buf = Path::new(path);
            if !path_buf.exists() {
                println!("Warning: Path '{}' does not exist", path);
            }

            let canonical_path = if path_buf.exists() {
                path_buf.canonicalize()
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
                },
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
```

#### Paso 6.4: Crear commands/hello.rs y commands/version.rs
```rust
// src/commands/hello.rs
use anyhow::Result;

pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    let name = matches.get_one::<String>("name").unwrap();
    println!("Hello, {}!", name);
    Ok(())
}
```

```rust
// src/commands/version.rs
use anyhow::Result;

pub fn execute() -> Result<()> {
    println!("msc version {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
```

#### Paso 6.5: Crear commands/list.rs
```rust
// src/commands/list.rs
use anyhow::Result;
use std::path::Path;
use colored::Colorize;
use crate::core::file_scanner::FileScanner;
use crate::git::{get_git_status_for_file, is_gitignored, apply_git_colors};
use crate::ui::{format_size, format_time, format_permissions};
use crate::utils::get_file_icon;

pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    let path = matches.get_one::<String>("path")
        .map(|s| s.as_str())
        .unwrap_or(".");
    
    let show_hidden = matches.get_flag("all");
    let show_details = matches.get_flag("long");
    
    log::info!("Listing directory: {}", path);
    
    let path = Path::new(path);
    let scanner = FileScanner::new(path)?;
    
    // Scan entries
    let entries = scanner.scan(show_hidden)?;
    
    if entries.is_empty() {
        println!("{}", "Directory is empty".yellow());
        return Ok(());
    }
    
    // Display
    for entry in entries {
        let git_status = get_git_status_for_file(
            scanner.git_status(),
            &entry.path,
            scanner.path()
        );
        
        let is_ignored = is_gitignored(
            scanner.gitignore(),
            &entry.path,
            entry.is_dir
        );
        
        // Build display string
        let icon = if entry.is_dir {
            "📁"
        } else {
            get_file_icon(&entry.name)
        };
        
        let name_display = if entry.is_dir {
            format!("{}/", entry.name)
        } else {
            entry.name.clone()
        };
        
        let colored_name = apply_git_colors(
            name_display,
            &git_status,
            entry.is_dir,
            is_ignored
        );
        
        if show_details {
            let size = format_size(entry.size);
            let time = format_time(entry.metadata.modified()?);
            let perms = format_permissions(&entry.metadata);
            
            println!("{} {} {:>8} {} {}",
                perms.dimmed(),
                time.dimmed(),
                size.yellow(),
                icon,
                colored_name
            );
        } else {
            println!("{}  {}", icon, colored_name);
        }
    }
    
    log::debug!("Listed {} entries", entries.len());
    Ok(())
}
```

#### Paso 6.6: Actualizar src/commands/mod.rs
```rust
// src/commands/mod.rs
pub mod hello;
pub mod version;
pub mod config;
pub mod workspace;
pub mod clean_temp;
pub mod list;  // ✅ Incluido en migración

// Re-exports para facilitar uso
pub use hello::execute as hello;
pub use version::execute as version;
pub use list::execute as list;
```

---
