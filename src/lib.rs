// MSC Library - Public API

// Re-export error types
pub mod error;
pub use error::{MscError, Result};

// Module declarations
pub mod commands;
pub mod core;
pub mod ui;
pub mod platform;
pub mod git;
pub mod utils;

// Re-export commonly used types
pub use core::config::Config;

// Initialize logging
pub fn init_logging() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();
}
