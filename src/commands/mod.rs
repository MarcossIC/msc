// Command handlers module
pub mod hello;
pub mod version;
pub mod config;
pub mod workspace;
pub mod clean_temp;
pub mod list;

// Re-exports for cleaner imports
pub use hello::execute as hello;
pub use version::execute as version;
pub use list::execute as list;
