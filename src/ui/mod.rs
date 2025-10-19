// UI and formatting module

pub mod formatters;
pub mod progress;
pub mod prompts;

// Re-export commonly used items for cleaner imports
pub use formatters::{format_permissions, format_size, format_time};
pub use progress::{clear_line, show_progress_bar};
pub use prompts::{bold, confirm, dimmed, error, info, success, warn};
