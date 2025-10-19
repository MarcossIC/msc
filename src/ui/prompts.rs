// UI prompts and user interaction module

use colored::Colorize;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{self, ClearType},
};
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

/// Ask user for confirmation with robust error handling and retry logic
///
/// # Arguments
/// * `prompt` - The prompt message to display
/// * `max_attempts` - Maximum number of retry attempts for IO errors (default 3)
///
/// # Returns
/// * `Ok(true)` - User confirmed (y/yes)
/// * `Ok(false)` - User declined (n/no or any other input)
/// * `Err` - IO error after max attempts
pub fn read_confirmation(prompt: &str, max_attempts: u32) -> anyhow::Result<bool> {
    for attempt in 1..=max_attempts {
        print!("{}", prompt.white().bold());
        io::stdout().flush()?;

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let response = input.trim().to_lowercase();
                return Ok(response == "y" || response == "yes");
            }
            Err(e) if attempt < max_attempts => {
                println!(
                    "{}",
                    format!(
                        "Error reading input (attempt {}/{}): {}",
                        attempt, max_attempts, e
                    )
                    .yellow()
                );
                println!("{}", "Retrying...".dimmed());
                continue;
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to read confirmation after {} attempts: {}",
                    max_attempts,
                    e
                ));
            }
        }
    }
    unreachable!()
}

/// Ask user for exact string confirmation (case-sensitive)
/// Used for high-risk operations requiring explicit confirmation
pub fn read_exact_confirmation(prompt: &str, expected: &str) -> anyhow::Result<bool> {
    print!("{}", prompt.white().bold());
    io::stdout().flush()?;

    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_) => Ok(input.trim() == expected),
        Err(e) => {
            println!();
            println!("{}", format!("Error reading input: {}", e).red());
            println!("{}", "Operation cancelled for safety.".yellow());
            Err(anyhow::anyhow!("Failed to read user confirmation: {}", e))
        }
    }
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

/// Interactive selection from a list of items
/// Returns the index of the selected item, or None if cancelled
pub fn select_from_list(title: &str, items: &[String]) -> io::Result<Option<usize>> {
    if items.is_empty() {
        return Ok(None);
    }

    let mut selected_index = 0;
    let mut stdout = io::stdout();

    println!("\n{}", "Initializing interactive selection...".dimmed());
    println!("{}", "Press any key to continue...".dimmed());
    stdout.flush()?;

    // Enable raw mode to capture key events
    terminal::enable_raw_mode().map_err(|e| {
        io::Error::other(format!(
            "Failed to enable raw mode: {}. Try running in a different terminal.",
            e
        ))
    })?;

    // Clear any pending events in the buffer
    while event::poll(std::time::Duration::from_millis(0))? {
        let _ = event::read()?;
    }

    // Ensure we disable raw mode on any exit path
    let result = run_selection_loop(title, items, &mut selected_index, &mut stdout);

    // Always disable raw mode, even if there was an error
    let _ = terminal::disable_raw_mode();

    // Don't clear the entire screen, just clear the selection UI and move back up
    // Print newlines to separate from next output
    println!("\n");

    result
}

/// Internal function that runs the selection loop
fn run_selection_loop(
    title: &str,
    items: &[String],
    selected_index: &mut usize,
    stdout: &mut io::Stdout,
) -> io::Result<Option<usize>> {
    loop {
        // Clear screen and reset cursor
        execute!(
            stdout,
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )?;

        // Print title
        println!("{}\r", title.white().bold());
        println!("\r");
        println!(
            "{}\r",
            "Use ↑/↓ arrows to navigate, Enter to select, Esc to cancel".dimmed()
        );
        println!("\r");

        // Print items
        for (index, item) in items.iter().enumerate() {
            // Clean path for display
            let cleaned_path = item.strip_prefix("\\\\?\\").unwrap_or(item);

            if index == *selected_index {
                // Highlight selected item
                println!("  {} {}\r", "→".green().bold(), cleaned_path.green().bold());
            } else {
                println!("    {}\r", cleaned_path.dimmed());
            }
        }

        stdout.flush()?;

        // Wait for and handle key events - block until we get a real keyboard event
        loop {
            match event::read() {
                Ok(Event::Key(KeyEvent { code, .. })) => {
                    match code {
                        KeyCode::Up => {
                            // Circular navigation: if at first item, go to last
                            if *selected_index == 0 {
                                *selected_index = items.len() - 1;
                            } else {
                                *selected_index -= 1;
                            }
                            // Clear any pending events to avoid skipping items
                            while event::poll(std::time::Duration::from_millis(0))? {
                                let _ = event::read()?;
                            }
                            break; // Redraw
                        }
                        KeyCode::Down => {
                            // Circular navigation: if at last item, go to first
                            if *selected_index >= items.len() - 1 {
                                *selected_index = 0;
                            } else {
                                *selected_index += 1;
                            }
                            // Clear any pending events to avoid skipping items
                            while event::poll(std::time::Duration::from_millis(0))? {
                                let _ = event::read()?;
                            }
                            break; // Redraw
                        }
                        KeyCode::Enter => {
                            return Ok(Some(*selected_index));
                        }
                        KeyCode::Esc
                        | KeyCode::Char('q')
                        | KeyCode::Char('Q')
                        | KeyCode::Char('c')
                        | KeyCode::Char('C') => {
                            return Ok(None);
                        }
                        _ => {
                            // Ignore other keys, keep waiting
                        }
                    }
                }
                Ok(_) => {
                    // Ignore non-keyboard events (mouse, resize, etc.)
                }
                Err(e) => {
                    return Err(io::Error::other(format!(
                        "Error reading keyboard input: {}",
                        e
                    )))
                }
            }
        }
    }
}
