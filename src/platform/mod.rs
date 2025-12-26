// Platform-specific code module

pub mod elevation;
pub mod fs;
pub mod gpu;
pub mod temp_dirs;
pub mod system;

#[cfg(windows)]
pub mod system_info_windows;

#[cfg(windows)]
pub mod nvidia_nvml;

// Re-exports para imports limpios
pub use elevation::{elevate_and_rerun, ensure_elevated, is_elevated, simulate_elevation_command};
pub use fs::is_hidden;
pub use temp_dirs::{get_default_temp_directories, get_recycle_bin_directory};
