pub mod battery;
pub mod collector;
pub mod cpu;
pub mod gpu;
pub mod memory;
pub mod memory_prediction;
pub mod motherboard;
pub mod network;
pub mod os;
pub mod power;
pub mod storage;
pub mod types;

pub use collector::collect_system_info;
pub use types::*;
