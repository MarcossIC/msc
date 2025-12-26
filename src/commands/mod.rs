// Command handlers module
pub mod alias;
pub mod clean;
pub mod completions;
pub mod config;
pub mod hello;
pub mod list;
pub mod sys;
pub mod update;
pub mod vedit;
pub mod version;
pub mod vget;
pub mod wget;
pub mod workspace;

// Re-exports for cleaner imports
pub use hello::execute as hello;
pub use list::execute as list;
pub use version::execute as version;
