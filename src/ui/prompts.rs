// UI prompts and user interaction module

use colored::Colorize;
use std::io::{self, Write};

/// Ask user for yes/no confirmation
pub fn confirm(message: &str) -> io::Result<bool> {
    print!("{} ", message.white().bold());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let response = input.trim().to_lowercase();
    Ok(response == "y" || response == "yes")
}

/// Display a warning message
pub fn warn(message: &str) {
    println!("{}", format!("⚠️  Warning: {}", message).yellow().bold());
}

/// Display an info message
pub fn info(message: &str) {
    println!("{}", message.cyan());
}

/// Display a success message
pub fn success(message: &str) {
    println!("{}", message.green().bold());
}

/// Display an error message
pub fn error(message: &str) {
    println!("{}", message.red().bold());
}

/// Display a dimmed/secondary message
pub fn dimmed(message: &str) {
    println!("{}", message.dimmed());
}

/// Display a bold white message
pub fn bold(message: &str) {
    println!("{}", message.white().bold());
}
