// Git integration module

pub mod colors;
pub mod ignore;
pub mod status;

// Re-export commonly used items
pub use colors::apply_git_colors;
pub use ignore::{is_gitignored, load_gitignore};
pub use status::{get_git_status_for_file, load_git_status, GitStatus};
