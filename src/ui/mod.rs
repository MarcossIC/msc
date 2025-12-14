// UI and formatting module

pub mod formatters;
pub mod monitor_tui;
pub mod progress;
pub mod prompts;
pub mod system_formatters;

// Re-export commonly used items for cleaner imports
pub use formatters::{format_permissions, format_size, format_time};
pub use progress::{clear_line, show_progress_bar};
pub use prompts::{
    bold, confirm, dimmed, error, info, read_confirmation, read_exact_confirmation,
    select_from_list, success, warn,
};
