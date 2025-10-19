// Core business logic module

pub mod cleaner;
pub mod config;
pub mod file_scanner;
pub mod workspace;

// Re-export commonly used items
pub use cleaner::{CleanupStats, TempCleaner};
pub use config::Config;
pub use file_scanner::{FileScanner, ScanEntry};
pub use workspace::WorkspaceManager;
