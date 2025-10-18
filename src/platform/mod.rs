// Platform-specific code module

pub mod elevation;
pub mod temp_dirs;
pub mod fs;

// Re-exports para imports limpios
pub use elevation::{is_elevated, elevate_and_rerun, ensure_elevated};
pub use temp_dirs::get_temp_directories;
pub use fs::is_hidden;
