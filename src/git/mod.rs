// Git integration module

pub mod status;
pub mod ignore;
pub mod colors;

// Re-export commonly used items
pub use status::{GitStatus, load_git_status, get_git_status_for_file};
pub use ignore::{load_gitignore, is_gitignored};
pub use colors::apply_git_colors;
