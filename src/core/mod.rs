// Core business logic module

pub mod alias;
pub mod alias_generator;
pub mod cleaner;
pub mod config;
pub mod file_scanner;
pub mod path_manager;
pub mod path_validator;
pub mod validation;
pub mod workspace;
pub mod yt_dlp_manager;

// Re-export commonly used items
pub use alias::{Alias, AliasConfig};
pub use alias_generator::{get_generator, AliasGenerator};
pub use cleaner::{CleanupStats, TempCleaner};
pub use config::Config;
pub use file_scanner::{FileScanner, ScanEntry};
pub use path_manager::PathManager;
pub use path_validator::{PathValidator, ValidationResult};
pub use workspace::WorkspaceManager;
pub use yt_dlp_manager::YtDlpManager;
