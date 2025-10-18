// UI and formatting module

pub mod formatters;
pub mod prompts;
pub mod progress;

// Re-export commonly used items for cleaner imports
pub use formatters::{format_size, format_time, format_permissions};
pub use prompts::{confirm, warn, info, success, error, dimmed, bold};
pub use progress::{show_progress_bar, clear_line};
