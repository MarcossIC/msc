// Re-exports from the new modular structure for backward compatibility
// This file maintains the old import paths while the code is organized in system/windows/

pub use super::system::windows::{
    DiskDetailsWindows,
    StorageSlots,
    get_disk_details,
    get_disk_type,
    get_available_storage_slots,
};
