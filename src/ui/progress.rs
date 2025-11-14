// Progress bar and progress indicators module
// This is a placeholder for future progress bar functionality

use colored::Colorize;
use std::io::{self, Write};

/// Display a simple progress bar
///
/// # Arguments
/// * `processed` - Number of items processed
/// * `total` - Total number of items
/// * `prefix` - Text to display before the progress bar
pub fn show_progress_bar(processed: usize, total: usize, prefix: &str) {
    let percentage = if total > 0 {
        (processed as f64 / total as f64 * 100.0) as usize
    } else {
        0
    };

    let bar_length: usize = 30;
    let filled = if total > 0 {
        (percentage as f64 / 100.0 * bar_length as f64) as usize
    } else {
        0
    };
    let empty = bar_length.saturating_sub(filled);

    print!(
        "\r{} [{}{}] {}% ({}/{}) ",
        prefix.white(),
        "=".repeat(filled).green(),
        " ".repeat(empty),
        percentage,
        processed,
        total
    );

    io::stdout().flush().ok();
}

/// Clear the current line (useful for progress bars)
pub fn clear_line() {
    print!("\r{}\r", " ".repeat(80));
    io::stdout().flush().ok();
}
