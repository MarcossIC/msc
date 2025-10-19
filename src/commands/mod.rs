// Command handlers module
pub mod clean_temp;
pub mod config;
pub mod hello;
pub mod list;
pub mod version;
pub mod workspace;

// Re-exports for cleaner imports
pub use hello::execute as hello;
pub use list::execute as list;
pub use version::execute as version;
