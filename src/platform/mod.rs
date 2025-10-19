// Platform-specific code module

pub mod elevation;
pub mod fs;
pub mod temp_dirs;

// Re-exports para imports limpios
pub use elevation::{elevate_and_rerun, ensure_elevated, is_elevated};
pub use fs::is_hidden;
pub use temp_dirs::{get_default_temp_directories, get_recycle_bin_directory};
