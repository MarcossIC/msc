use crate::git::{
    apply_git_colors, get_git_status_for_file, is_gitignored, load_git_status, load_gitignore,
};
use crate::platform::is_hidden;
use crate::ui::{format_permissions, format_size, format_time};
use crate::utils::icons::get_file_icon;
use anyhow::Result;
use colored::*;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn execute(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("deep", sub_matches)) => {
            let path = sub_matches
                .get_one::<String>("path")
                .map(|s| s.as_str())
                .unwrap_or(".");
            let show_all = sub_matches.get_flag("all");
            let depth = *sub_matches.get_one::<u32>("depth").unwrap();

            list_deep(path, show_all, depth)
        }
        _ => {
            let path = matches
                .get_one::<String>("path")
                .map(|s| s.as_str())
                .unwrap_or(".");
            let show_all = matches.get_flag("all");
            let is_deep = matches.get_flag("deep");
            let is_long = matches.get_flag("long");

            if is_long {
                list_long(
                    path,
                    show_all,
                    is_deep,
                    if is_deep {
                        *matches.get_one::<u32>("depth").unwrap()
                    } else {
                        0
                    },
                )
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
        println!(
            "{}",
            format!("Error: Directory '{}' does not exist", path).red()
        );
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

    let path_buf = dir_path
        .canonicalize()
        .unwrap_or_else(|_| dir_path.to_path_buf());
    let canonical_path = path_buf.to_string_lossy();
    let clean_path = canonical_path
        .strip_prefix("\\\\?\\")
        .unwrap_or(&canonical_path);

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
        println!(
            "{}",
            format!("Error: Directory '{}' does not exist", path).red()
        );
        return Ok(());
    }

    if !dir_path.is_dir() {
        println!("{}", format!("Error: '{}' is not a directory", path).red());
        return Ok(());
    }

    let path_buf = dir_path
        .canonicalize()
        .unwrap_or_else(|_| dir_path.to_path_buf());
    let canonical_path = path_buf.to_string_lossy();
    let clean_path = canonical_path
        .strip_prefix("\\\\?\\")
        .unwrap_or(&canonical_path);

    println!(
        "{} {} {}",
        "Directory:".white(),
        clean_path.cyan().bold(),
        format!("(depth: {})", max_depth).dimmed()
    );
    println!();

    list_recursive(dir_path, show_all, 0, max_depth)?;

    Ok(())
}

fn list_recursive(
    dir_path: &Path,
    show_all: bool,
    current_depth: u32,
    max_depth: u32,
) -> Result<()> {
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
                && list_recursive(full_path, show_all, current_depth + 1, max_depth).is_err()
            {
                println!(
                    "{}  {}",
                    indent,
                    format!("Error reading directory: {}", name).red().dimmed()
                );
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
        println!(
            "{}",
            format!("Error: Directory '{}' does not exist", path).red()
        );
        return Ok(());
    }

    if !dir_path.is_dir() {
        println!("{}", format!("Error: '{}' is not a directory", path).red());
        return Ok(());
    }

    let path_buf = dir_path
        .canonicalize()
        .unwrap_or_else(|_| dir_path.to_path_buf());
    let canonical_path = path_buf.to_string_lossy();
    let clean_path = canonical_path
        .strip_prefix("\\\\?\\")
        .unwrap_or(&canonical_path);

    if is_deep {
        println!(
            "{} {} {}",
            "Directory:".white(),
            clean_path.cyan().bold(),
            format!("(depth: {}, long format)", max_depth).dimmed()
        );
    } else {
        println!(
            "{} {} {}",
            "Directory:".white(),
            clean_path.cyan().bold(),
            "(long format)".dimmed()
        );
    }
    println!();

    // Header (Name column: 38 total = icon(2) + space(1) + name text(35))
    println!(
        "{:<38} │ {:<9} │ {:<18} │ {:<18} │ {:<12}",
        "Name".white().bold(),
        "Size".white().bold(),
        "Created".white().bold(),
        "Modified".white().bold(),
        "Permissions".white().bold()
    );
    println!(
        "{}─┼─{}─┼─{}─┼─{}─┼─{}",
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
        let icon = if *is_dir {
            "📂"
        } else {
            get_file_icon(name).trim_end()
        };
        let size = if *is_dir {
            "-".to_string()
        } else {
            format_size(metadata.len())
        };
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
                if w + cw > name_available.saturating_sub(3) {
                    // keep room for "..."
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

        let size_color = if is_dimmed {
            size_padded.bright_black()
        } else {
            size_padded.yellow()
        };
        let created_color = if is_dimmed {
            created_padded.bright_black()
        } else {
            created_padded.cyan()
        };
        let modified_color = if is_dimmed {
            modified_padded.bright_black()
        } else {
            modified_padded.green()
        };
        let permissions_color = if is_dimmed {
            permissions_padded.bright_black()
        } else {
            permissions_padded.magenta()
        };

        println!(
            "{} │ {} │ {} │ {} │ {}",
            colored_name, size_color, created_color, modified_color, permissions_color
        );
    }

    Ok(())
}

fn list_long_recursive(
    dir_path: &Path,
    show_all: bool,
    current_depth: u32,
    max_depth: u32,
) -> Result<()> {
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
        let icon = if *is_dir {
            "📂"
        } else {
            get_file_icon(name).trim_end()
        };
        let size = if *is_dir {
            "-".to_string()
        } else {
            format_size(metadata.len())
        };
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

        let size_color = if is_dimmed {
            size_padded.bright_black()
        } else {
            size_padded.yellow()
        };
        let created_color = if is_dimmed {
            created_padded.bright_black()
        } else {
            created_padded.cyan()
        };
        let modified_color = if is_dimmed {
            modified_padded.bright_black()
        } else {
            modified_padded.green()
        };
        let permissions_color = if is_dimmed {
            permissions_padded.bright_black()
        } else {
            permissions_padded.magenta()
        };

        println!(
            "{} │ {} │ {} │ {} │ {}",
            colored_name, size_color, created_color, modified_color, permissions_color
        );

        if *is_dir
            && current_depth < max_depth
            && list_long_recursive(full_path, show_all, current_depth + 1, max_depth).is_err()
        {
            let indent_error = "  ".repeat((current_depth + 1) as usize);
            println!(
                "{}  {}",
                indent_error,
                format!("Error reading directory: {}", name).red().dimmed()
            );
        }
    }

    Ok(())
}
