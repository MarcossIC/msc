use clap::{Arg, Command};
use anyhow::Result;
use std::path::Path;
use std::fs;
use std::time::SystemTime;
use colored::*;
use unicode_width::{UnicodeWidthStr, UnicodeWidthChar};

// Use modules from the library
use msc::core::config::Config;
use msc::ui::{format_size, format_time, format_permissions};
use msc::utils::icons::get_file_icon;
use msc::platform::{is_elevated, elevate_and_rerun, get_temp_directories, is_hidden};
use msc::git::{load_git_status, get_git_status_for_file, load_gitignore, is_gitignored, apply_git_colors};

fn main() -> Result<()> {
    let matches = Command::new("msc")
        .version("0.1.0")
        .author("Marco")
        .about("A custom CLI tool")
        .disable_version_flag(true)
        .arg(
            Arg::new("version")
                .short('v')
                .short_alias('V')
                .long("version")
                .help("Print version information")
                .action(clap::ArgAction::SetTrue)
        )
        .subcommand(
            Command::new("hello")
                .about("Says hello")
                .arg(
                    Arg::new("name")
                        .short('n')
                        .long("name")
                        .value_name("NAME")
                        .help("Name to greet")
                        .default_value("World")
                )
        )
        .subcommand(
            Command::new("version")
                .about("Shows version information")
        )
        .subcommand(
            Command::new("set")
                .about("Set configuration values (use 'msc set --help' for subcommands)")
                .long_about("Set configuration values\n\nUSAGE:\n    msc set <SUBCOMMAND>\n\nSUBCOMMANDS:\n    work    Set work directory path\n\nFor more information try --help")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("work")
                        .about("Set work directory path")
                        .arg(
                            Arg::new("path")
                                .help("Path to the work directory")
                                .required(true)
                                .index(1)
                        )
                )
        )
        .subcommand(
            Command::new("get")
                .about("Get configuration values (use 'msc get --help' for subcommands)")
                .long_about("Get configuration values\n\nUSAGE:\n    msc get <SUBCOMMAND>\n\nSUBCOMMANDS:\n    work    Get work directory path\n\nFor more information try --help")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("work")
                        .about("Get work directory path")
                )
        )
        .subcommand(
            Command::new("work")
                .about("Manage workspaces (use 'msc work --help' for subcommands)")
                .long_about("Manage workspaces within your work directory\n\nUSAGE:\n    msc work <SUBCOMMAND>\n\nSUBCOMMANDS:\n    map     Map project folders in work directory as workspaces\n    list    List all registered workspaces\n\nFor more information try --help")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("map")
                        .about("Map project folders as workspaces")
                )
                .subcommand(
                    Command::new("list")
                        .about("List all registered workspaces")
                )
        )
        .subcommand(
            Command::new("clean-temp")
                .about("Clean temporary files from the system")
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .help("Show what would be deleted without actually deleting")
                        .action(clap::ArgAction::SetTrue)
                )
        )
        .subcommand(
            Command::new("list")
                .about("List files and directories (use 'msc list --help' for subcommands)")
                .arg(
                    Arg::new("path")
                        .help("Directory to list (defaults to current directory)")
                        .index(1)
                )
                .arg(
                    Arg::new("all")
                        .short('a')
                        .long("all")
                        .help("Show hidden files")
                        .action(clap::ArgAction::SetTrue)
                )
                .arg(
                    Arg::new("deep")
                        .short('d')
                        .long("deep")
                        .help("List files recursively (default depth: 1)")
                        .action(clap::ArgAction::SetTrue)
                )
                .arg(
                    Arg::new("depth")
                        .long("depth")
                        .help("Maximum depth to traverse when using --deep")
                        .value_parser(clap::value_parser!(u32))
                        .default_value("1")
                        .requires("deep")
                )
                .arg(
                    Arg::new("long")
                        .short('l')
                        .long("long")
                        .help("Use long listing format (table view)")
                        .action(clap::ArgAction::SetTrue)
                )
                .subcommand(
                    Command::new("deep")
                        .about("List files and directories recursively")
                        .arg(
                            Arg::new("path")
                                .help("Directory to list (defaults to current directory)")
                                .index(1)
                        )
                        .arg(
                            Arg::new("all")
                                .short('a')
                                .long("all")
                                .help("Show hidden files")
                                .action(clap::ArgAction::SetTrue)
                        )
                        .arg(
                            Arg::new("depth")
                                .short('d')
                                .long("depth")
                                .help("Maximum depth to traverse (default: 1)")
                                .value_parser(clap::value_parser!(u32))
                                .default_value("1")
                        )
                )
        )
        .get_matches();

    if matches.get_flag("version") {
        println!("msc version {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    match matches.subcommand() {
        Some(("hello", sub_matches)) => {
            let name = sub_matches.get_one::<String>("name").unwrap();
            println!("Hello, {}!", name);
        }
        Some(("version", _)) => {
            println!("msc version {}", env!("CARGO_PKG_VERSION"));
        }
        Some(("set", sub_matches)) => {
            handle_set_command(sub_matches)?;
        }
        Some(("get", sub_matches)) => {
            handle_get_command(sub_matches)?;
        }
        Some(("work", sub_matches)) => {
            handle_work_command(sub_matches)?;
        }
        Some(("clean-temp", sub_matches)) => {
            handle_clean_temp_command(sub_matches)?;
        }
        Some(("list", sub_matches)) => {
            handle_list_command(sub_matches)?;
        }
        _ => {
            println!("Welcome to msc CLI!");
            println!("Use 'msc --help' for more information.");
        }
    }

    Ok(())
}

fn handle_set_command(matches: &clap::ArgMatches) -> Result<()> {
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

fn handle_get_command(matches: &clap::ArgMatches) -> Result<()> {
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

fn handle_work_command(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("map", _)) => {
            let mut config = Config::load()?;

            let work_path = match config.get_work_path() {
                Some(path) => path.clone(),
                None => {
                    println!("{}", "No work directory configured.".yellow());
                    println!();
                    println!("{}", "To set a work directory first, run:".white());
                    println!("  {}", "msc set work <path>".cyan().bold());
                    println!();
                    println!("{}", "Example:".dimmed());
                    println!("  {}", "msc set work C:\\Users\\marco\\projects".dimmed());
                    return Ok(());
                }
            };

            let work_dir = Path::new(&work_path);
            if !work_dir.exists() {
                println!("{}", format!("Error: Work directory '{}' does not exist", work_path).red());
                return Ok(());
            }

            if !work_dir.is_dir() {
                println!("{}", format!("Error: '{}' is not a directory", work_path).red());
                return Ok(());
            }

            println!("{}", "Mapping workspaces...".cyan());
            println!();

            config.clear_workspaces();
            let entries = fs::read_dir(work_dir)?;
            let mut count = 0;

            for entry in entries {
                let entry = entry?;
                let file_name = entry.file_name().to_string_lossy().to_string();

                if entry.file_type()?.is_dir() && !file_name.starts_with('.') {
                    let full_path = entry.path();
                    let canonical_path = full_path.canonicalize()
                        .unwrap_or(full_path)
                        .to_string_lossy()
                        .to_string();

                    config.add_workspace(file_name.clone(), canonical_path);
                    println!("  {} {}", "✓".green(), file_name.cyan());
                    count += 1;
                }
            }

            config.save()?;

            println!();
            println!("{} {}", "Successfully mapped".green().bold(), format!("{} workspace(s)", count).yellow().bold());
        }
        Some(("list", _)) => {
            let config = Config::load()?;
            let workspaces = config.get_workspaces();

            if workspaces.is_empty() {
                println!("{}", "No workspaces found. Use 'msc work map' to map your project folders.".yellow());
                return Ok(());
            }

            println!("{} {}", "Workspaces:".white().bold(), format!("({} total)", workspaces.len()).dimmed());
            println!();

            let mut sorted_workspaces: Vec<_> = workspaces.iter().collect();
            sorted_workspaces.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

            for (name, path) in sorted_workspaces {
                let cleaned_path = path.strip_prefix("\\\\?\\").unwrap_or(path);
                println!("  {} {}", "📂".to_string().blue().bold(), name.cyan().bold());
                println!("     {}", cleaned_path.dimmed());
            }
        }
        _ => {
            println!("Use 'msc work --help' for more information.");
        }
    }

    Ok(())
}

fn handle_clean_temp_command(matches: &clap::ArgMatches) -> Result<()> {
    let dry_run = matches.get_flag("dry-run");

    if dry_run {
        println!("{}", "DRY RUN MODE - No files will be deleted".yellow().bold());
        println!();
    }

    println!("{}", "Starting cleanup of temporary files...".cyan().bold());
    println!();

    // Get temp directories based on OS
    let temp_dirs = get_temp_directories();

    if temp_dirs.is_empty() {
        println!("{}", "No temp directories found.".yellow());
        return Ok(());
    }

    println!("{}", "Directories to clean:".white().bold());
    for (idx, dir) in temp_dirs.iter().enumerate() {
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

    let mut total_files = 0usize;
    let mut total_size = 0u64;
    let mut deleted_files = 0usize;
    let mut deleted_size = 0u64;
    let mut failed_files = 0usize;

    // First pass: count files
    println!("{}", "Scanning temporary files...".dimmed());
    for temp_dir in &temp_dirs {
        count_files_recursive(Path::new(temp_dir), &mut total_files, &mut total_size);
    }

    if total_files == 0 {
        println!("{}", "No temporary files found to clean.".green());
        return Ok(());
    }

    println!("{} {} files ({}) found",
        "Found:".white().bold(),
        total_files.to_string().yellow().bold(),
        format_size(total_size).yellow().bold()
    );
    println!();

    if dry_run {
        println!("{}", "Files that would be deleted:".white().bold());
        println!();
    } else {
        println!("{}", "Cleaning...".cyan().bold());
        println!();
    }

    let mut processed = 0usize;

    // Second pass: delete files
    for temp_dir in &temp_dirs {
        delete_files_recursive(
            Path::new(temp_dir),
            &mut processed,
            total_files,
            &mut deleted_files,
            &mut deleted_size,
            &mut failed_files,
            dry_run,
        );
    }

    println!();
    println!();
    println!("{}", "─".repeat(50));
    println!("{}", "Cleanup Summary".white().bold());
    println!("{}", "─".repeat(50));

    if dry_run {
        println!("{} {}", "Would delete:".white(), format!("{} files", deleted_files).yellow().bold());
        println!("{} {}", "Space to recover:".white(), format_size(deleted_size).yellow().bold());
    } else {
        println!("{} {}", "Deleted:".green().bold(), format!("{} files", deleted_files).yellow().bold());
        println!("{} {}", "Space recovered:".green().bold(), format_size(deleted_size).yellow().bold());

        if failed_files > 0 {
            println!("{} {} (files in use or protected)",
                "Failed:".red().bold(),
                format!("{} files", failed_files).red()
            );
        }
    }

    println!();

    Ok(())
}

fn count_files_recursive(dir: &Path, total_files: &mut usize, total_size: &mut u64) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    *total_files += 1;
                    *total_size += metadata.len();
                } else if metadata.is_dir() {
                    // Recursively count files in subdirectories
                    count_files_recursive(&entry.path(), total_files, total_size);
                }
            }
        }
    }
}

fn delete_files_recursive(
    dir: &Path,
    processed: &mut usize,
    total_files: usize,
    deleted_files: &mut usize,
    deleted_size: &mut u64,
    failed_files: &mut usize,
    dry_run: bool,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    *processed += 1;
                    let file_path = entry.path();
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    let file_size = metadata.len();

                    // Update progress
                    let percentage = (*processed as f64 / total_files as f64 * 100.0) as usize;
                    let bar_length = 30;
                    let filled = (percentage as f64 / 100.0 * bar_length as f64) as usize;
                    let empty = bar_length - filled;

                    print!("\r{} [{}{}] {}% ({}/{}) ",
                        "Progress:".white(),
                        "=".repeat(filled).green(),
                        " ".repeat(empty),
                        percentage,
                        processed,
                        total_files
                    );

                    use std::io::Write;
                    std::io::stdout().flush().ok();

                    if dry_run {
                        if *processed % 50 == 0 || *processed <= 10 {
                            println!();
                            println!("  {} {} ({})",
                                "Would delete:".dimmed(),
                                file_name.dimmed(),
                                format_size(file_size).dimmed()
                            );
                        }
                        *deleted_files += 1;
                        *deleted_size += file_size;
                    } else {
                        match fs::remove_file(&file_path) {
                            Ok(_) => {
                                *deleted_files += 1;
                                *deleted_size += file_size;
                            }
                            Err(_) => {
                                *failed_files += 1;
                            }
                        }
                    }
                } else if metadata.is_dir() {
                    // Recursively delete files in subdirectories
                    delete_files_recursive(
                        &entry.path(),
                        processed,
                        total_files,
                        deleted_files,
                        deleted_size,
                        failed_files,
                        dry_run,
                    );
                }
            }
        }
    }
}


fn handle_list_command(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("deep", sub_matches)) => {
            let path = sub_matches.get_one::<String>("path")
                .map(|s| s.as_str())
                .unwrap_or(".");
            let show_all = sub_matches.get_flag("all");
            let depth = *sub_matches.get_one::<u32>("depth").unwrap();
            
            list_deep(path, show_all, depth)
        }
        _ => {
            let path = matches.get_one::<String>("path")
                .map(|s| s.as_str())
                .unwrap_or(".");
            let show_all = matches.get_flag("all");
            let is_deep = matches.get_flag("deep");
            let is_long = matches.get_flag("long");
            
            if is_long {
                list_long(path, show_all, is_deep, if is_deep { *matches.get_one::<u32>("depth").unwrap() } else { 0 })
            } else if is_deep {
                let depth = *matches.get_one::<u32>("depth").unwrap();
                list_deep(path, show_all, depth)
            } else {
                list_simple(path, show_all)
            }
        }
    }
}

fn list_simple(path: &str, show_all: bool) -> Result<()> {
    let dir_path = Path::new(path);
    
    if !dir_path.exists() {
        println!("{}", format!("Error: Directory '{}' does not exist", path).red());
        return Ok(());
    }
    
    if !dir_path.is_dir() {
        println!("{}", format!("Error: '{}' is not a directory", path).red());
        return Ok(());
    }
    
    let gitignore = load_gitignore(dir_path);
    let git_status_map = load_git_status(dir_path);
    let entries = fs::read_dir(dir_path)?;
    let mut items = Vec::new();
    
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        
        if !show_all && (file_name.starts_with('.') || is_hidden(&entry)) {
            continue;
        }
        
        let is_dir = entry.file_type()?.is_dir();
        let is_hidden = file_name.starts_with('.') || is_hidden(&entry);
        let is_ignored = is_gitignored(&gitignore, &entry.path(), is_dir);
        let git_status = get_git_status_for_file(&git_status_map, &entry.path(), dir_path);
        
        items.push((file_name, is_dir, is_hidden, is_ignored, git_status));
    }
    
    items.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    
    let path_buf = dir_path.canonicalize()
        .unwrap_or_else(|_| dir_path.to_path_buf());
    let canonical_path = path_buf.to_string_lossy();
    let clean_path = canonical_path.strip_prefix("\\\\?\\").unwrap_or(&canonical_path);
    
    println!("{} {}", "Directory:".white(), clean_path.cyan().bold());
    println!();
    
    if items.is_empty() {
        println!("{}", "Directory is empty".yellow().italic());
    } else {
        for (name, is_dir, is_hidden, is_ignored, git_status) in &items {
            let is_dimmed = *is_hidden || *is_ignored;
            
            if *is_dir {
                let colored_name = apply_git_colors(name.clone(), git_status, true, is_dimmed);
                println!("📂 {}", colored_name);
            } else {
                let icon = get_file_icon(name);
                let colored_name = apply_git_colors(name.clone(), git_status, false, is_dimmed);
                println!("{} {}", icon, colored_name);
            }
        }
    }
    
    Ok(())
}

fn list_deep(path: &str, show_all: bool, max_depth: u32) -> Result<()> {
    let dir_path = Path::new(path);
    
    if !dir_path.exists() {
        println!("{}", format!("Error: Directory '{}' does not exist", path).red());
        return Ok(());
    }
    
    if !dir_path.is_dir() {
        println!("{}", format!("Error: '{}' is not a directory", path).red());
        return Ok(());
    }
    
    let path_buf = dir_path.canonicalize()
        .unwrap_or_else(|_| dir_path.to_path_buf());
    let canonical_path = path_buf.to_string_lossy();
    let clean_path = canonical_path.strip_prefix("\\\\?\\").unwrap_or(&canonical_path);
    
    println!("{} {} {}", "Directory:".white(), clean_path.cyan().bold(), format!("(depth: {})", max_depth).dimmed());
    println!();
    
    list_recursive(dir_path, show_all, 0, max_depth)?;
    
    Ok(())
}

fn list_recursive(dir_path: &Path, show_all: bool, current_depth: u32, max_depth: u32) -> Result<()> {
    if current_depth > max_depth {
        return Ok(());
    }
    
    let gitignore = load_gitignore(dir_path);
    let git_status_map = load_git_status(dir_path);
    let entries = fs::read_dir(dir_path)?;
    let mut items = Vec::new();
    
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        
        if !show_all && (file_name.starts_with('.') || is_hidden(&entry)) {
            continue;
        }
        
        let is_dir = entry.file_type()?.is_dir();
        let is_hidden = file_name.starts_with('.') || is_hidden(&entry);
        let is_ignored = is_gitignored(&gitignore, &entry.path(), is_dir);
        
        items.push((file_name, is_dir, entry.path(), is_hidden, is_ignored));
    }
    
    items.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    
    for (name, is_dir, full_path, is_hidden, is_ignored) in &items {
        let indent = "  ".repeat(current_depth as usize);
        let is_dimmed = *is_hidden || *is_ignored;
        let git_status = get_git_status_for_file(&git_status_map, full_path, dir_path);
        
        if *is_dir {
            let colored_name = apply_git_colors(name.clone(), &git_status, true, is_dimmed);
            println!("{}📂 {}", indent, colored_name);
            if current_depth < max_depth
                && list_recursive(full_path, show_all, current_depth + 1, max_depth).is_err() {
                    println!("{}  {}", indent, format!("Error reading directory: {}", name).red().dimmed());
                }
        } else {
            let icon = get_file_icon(name);
            let colored_name = apply_git_colors(name.clone(), &git_status, false, is_dimmed);
            println!("{}{} {}", indent, icon, colored_name);
        }
    }
    
    Ok(())
}

fn list_long(path: &str, show_all: bool, is_deep: bool, max_depth: u32) -> Result<()> {
    let dir_path = Path::new(path);
    
    if !dir_path.exists() {
        println!("{}", format!("Error: Directory '{}' does not exist", path).red());
        return Ok(());
    }
    
    if !dir_path.is_dir() {
        println!("{}", format!("Error: '{}' is not a directory", path).red());
        return Ok(());
    }
    
    let path_buf = dir_path.canonicalize()
        .unwrap_or_else(|_| dir_path.to_path_buf());
    let canonical_path = path_buf.to_string_lossy();
    let clean_path = canonical_path.strip_prefix("\\\\?\\").unwrap_or(&canonical_path);
    
    if is_deep {
        println!("{} {} {}", "Directory:".white(), clean_path.cyan().bold(), format!("(depth: {}, long format)", max_depth).dimmed());
    } else {
        println!("{} {} {}", "Directory:".white(), clean_path.cyan().bold(), "(long format)".dimmed());
    }
    println!();
    
    // Header (Name column: 38 total = icon(2) + space(1) + name text(35))
    println!("{:<38} │ {:<9} │ {:<18} │ {:<18} │ {:<12}", 
        "Name".white().bold(), 
        "Size".white().bold(), 
        "Created".white().bold(), 
        "Modified".white().bold(), 
        "Permissions".white().bold()
    );
    println!("{}─┼─{}─┼─{}─┼─{}─┼─{}", 
        "─".repeat(38), 
        "─".repeat(9), 
        "─".repeat(18), 
        "─".repeat(18), 
        "─".repeat(12)
    );
    
    if is_deep {
        list_long_recursive(dir_path, show_all, 0, max_depth)?;
    } else {
        list_long_simple(dir_path, show_all, 0)?;
    }
    
    Ok(())
}

fn list_long_simple(dir_path: &Path, show_all: bool, indent_level: u32) -> Result<()> {
    let gitignore = load_gitignore(dir_path);
    let git_status_map = load_git_status(dir_path);
    let entries = fs::read_dir(dir_path)?;
    let mut items = Vec::new();
    
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        
        if !show_all && (file_name.starts_with('.') || is_hidden(&entry)) {
            continue;
        }
        
        let is_dir = entry.file_type()?.is_dir();
        let is_hidden = file_name.starts_with('.') || is_hidden(&entry);
        let is_ignored = is_gitignored(&gitignore, &entry.path(), is_dir);
        
        items.push((file_name, is_dir, entry.path(), is_hidden, is_ignored));
    }
    
    items.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    
    for (name, is_dir, full_path, is_hidden, is_ignored) in &items {
        let indent = "  ".repeat(indent_level as usize);
        let metadata = fs::metadata(full_path)?;
        let is_dimmed = *is_hidden || *is_ignored;
        let git_status = get_git_status_for_file(&git_status_map, full_path, dir_path);
        
        // Ensure we never keep stray spaces in the icon
        let icon = if *is_dir { "📂" } else { get_file_icon(name).trim_end() };
        let size = if *is_dir { "-".to_string() } else { format_size(metadata.len()) };
        let created = format_time(metadata.created().unwrap_or(SystemTime::UNIX_EPOCH));
        let modified = format_time(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH));
        let permissions = format_permissions(&metadata);
        
        // Name column: reserve exactly 35 chars for the name text (excluding icon and colors).
        // Truncate to 32 + "..." when it exceeds the limit. Then pad so that
        // indent + icon(2) + space(1) + name_text(<=35, padded) = 38 + indent_width total before the separator.
        const NAME_TEXT_WIDTH: usize = 35;
        let icon_width = icon.width();
        // Determine available width for the name text (no colors), excluding the indent.
        let indent_width = indent.width();
        let name_available = NAME_TEXT_WIDTH.saturating_sub(indent_width);

        // Truncate name respecting unicode display width
        let truncated_name = if name.width() > name_available {
            let mut out = String::new();
            let mut w = 0usize;
            for ch in name.chars() {
                let cw = ch.width().unwrap_or(0);
                if w + cw > name_available.saturating_sub(3) { // keep room for "..."
                    break;
                }
                out.push(ch);
                w += cw;
            }
            out.push_str("...");
            out
        } else {
            name.clone()
        };

        // Pad the name text area to the available width
        let name_text_width = truncated_name.width();
        let pad_spaces = name_available.saturating_sub(name_text_width);
        let padded_name_text = format!("{}{}", truncated_name, " ".repeat(pad_spaces));

        // Build full cell: indent + icon + spacer(s) + padded name text
        // Add one normal space plus a compensation space if icon renders as width 1
        let extra_icon_pad = 2usize.saturating_sub(icon_width);
        let spacer = format!("{}{}", " ", " ".repeat(extra_icon_pad));
        let padded_name = format!("{}{}{}{}", indent, icon, spacer, padded_name_text);
        
        let colored_name = apply_git_colors(padded_name, &git_status, *is_dir, is_dimmed);
        
        // Pad other columns to fixed widths
        let size_padded = format!("{:<9}", size);
        let created_padded = format!("{:<18}", created);
        let modified_padded = format!("{:<18}", modified);
        let permissions_padded = format!("{:<12}", permissions);
        
        let size_color = if is_dimmed { size_padded.bright_black() } else { size_padded.yellow() };
        let created_color = if is_dimmed { created_padded.bright_black() } else { created_padded.cyan() };
        let modified_color = if is_dimmed { modified_padded.bright_black() } else { modified_padded.green() };
        let permissions_color = if is_dimmed { permissions_padded.bright_black() } else { permissions_padded.magenta() };
        
        println!("{} │ {} │ {} │ {} │ {}", 
            colored_name, 
            size_color, 
            created_color, 
            modified_color, 
            permissions_color
        );
    }
    
    Ok(())
}

fn list_long_recursive(dir_path: &Path, show_all: bool, current_depth: u32, max_depth: u32) -> Result<()> {
    if current_depth > max_depth {
        return Ok(());
    }
    
    let gitignore = load_gitignore(dir_path);
    let git_status_map = load_git_status(dir_path);
    let entries = fs::read_dir(dir_path)?;
    let mut items = Vec::new();
    const NAME_TEXT_WIDTH: usize = 35;
    
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        
        if !show_all && (file_name.starts_with('.') || is_hidden(&entry)) {
            continue;
        }
        
        let is_dir = entry.file_type()?.is_dir();
        let is_hidden = file_name.starts_with('.') || is_hidden(&entry);
        let is_ignored = is_gitignored(&gitignore, &entry.path(), is_dir);
        
        items.push((file_name, is_dir, entry.path(), is_hidden, is_ignored));
    }
    
    items.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    
    for (name, is_dir, full_path, is_hidden, is_ignored) in &items {
        let indent = "  ".repeat(current_depth as usize);
        let metadata = fs::metadata(full_path)?;
        let is_dimmed = *is_hidden || *is_ignored;
        let git_status = get_git_status_for_file(&git_status_map, full_path, dir_path);
        
        // Normalize icon and compute other columns
        let icon = if *is_dir { "📂" } else { get_file_icon(name).trim_end() };
        let size = if *is_dir { "-".to_string() } else { format_size(metadata.len()) };
        let created = format_time(metadata.created().unwrap_or(SystemTime::UNIX_EPOCH));
        let modified = format_time(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH));
        let permissions = format_permissions(&metadata);

        // First column width logic identical to simple long listing
        
        let indent_width = indent.width();
        let name_available = NAME_TEXT_WIDTH.saturating_sub(indent_width);

        let truncated_name = if name.width() > name_available {
            let mut out = String::new();
            let mut w = 0usize;
            for ch in name.chars() {
                let cw = ch.width().unwrap_or(0);
                if w + cw > name_available.saturating_sub(3) {
                    break;
                }
                out.push(ch);
                w += cw;
            }
            out.push_str("...");
            out
        } else {
            name.clone()
        };

        let name_text_width = truncated_name.width();
        let pad_spaces = name_available.saturating_sub(name_text_width);
        let padded_name_text = format!("{}{}", truncated_name, " ".repeat(pad_spaces));
        // Add one normal space plus compensation so (icon width + extra) = 2
        let icon_width = icon.width();
        let extra_icon_pad = 2usize.saturating_sub(icon_width);
        let spacer = format!("{}{}", " ", " ".repeat(extra_icon_pad));
        let padded_name = format!("{}{}{}{}", indent, icon, spacer, padded_name_text);

        let colored_name = apply_git_colors(padded_name, &git_status, *is_dir, is_dimmed);

        let size_padded = format!("{:<9}", size);
        let created_padded = format!("{:<18}", created);
        let modified_padded = format!("{:<18}", modified);
        let permissions_padded = format!("{:<12}", permissions);

        let size_color = if is_dimmed { size_padded.bright_black() } else { size_padded.yellow() };
        let created_color = if is_dimmed { created_padded.bright_black() } else { created_padded.cyan() };
        let modified_color = if is_dimmed { modified_padded.bright_black() } else { modified_padded.green() };
        let permissions_color = if is_dimmed { permissions_padded.bright_black() } else { permissions_padded.magenta() };

        println!("{} │ {} │ {} │ {} │ {}", 
            colored_name,
            size_color,
            created_color,
            modified_color,
            permissions_color
        );

        if *is_dir && current_depth < max_depth
            && list_long_recursive(full_path, show_all, current_depth + 1, max_depth).is_err() {
                let indent_error = "  ".repeat((current_depth + 1) as usize);
                println!("{}  {}", indent_error, format!("Error reading directory: {}", name).red().dimmed());
            }
    }
    
    Ok(())
}

