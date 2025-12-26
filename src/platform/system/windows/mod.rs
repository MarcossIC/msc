// Windows system information modules

pub mod battery;
pub mod core;
pub mod cpu;
pub mod gpu;
pub mod mbo;
pub mod network;
pub mod ram;
pub mod storage;

// Re-exports for backward compatibility
pub use storage::{DiskDetailsWindows, StorageSlots, get_disk_details, get_disk_type, get_available_storage_slots};
