// Core business logic module

pub mod config;
pub mod cleaner;
pub mod workspace;
pub mod file_scanner;

// Re-export commonly used items
pub use config::Config;
pub use cleaner::{TempCleaner, CleanupStats};
pub use workspace::WorkspaceManager;
pub use file_scanner::{FileScanner, ScanEntry};
