// Windows system information modules

pub mod battery;
pub mod battery_ioctl;
pub mod cpu;
pub mod gpu;
pub mod mbo;
pub mod monitor;
pub mod network;
pub mod nvme_smart;
pub mod os;
pub mod pcie_link;
pub mod ram;
pub mod storage;
pub mod wifi;

// Re-exports for backward compatibility
pub use storage::{
    get_available_storage_slots, get_disk_details, get_disk_type, DiskDetailsWindows, StorageSlots,
};
