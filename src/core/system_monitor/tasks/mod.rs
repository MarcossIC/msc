//! Async tasks for collecting system metrics.
//!
//! Each subsystem runs in its own independent task with appropriate polling frequency.

mod battery;
mod cpu_memory;
mod disks;
mod global;
mod gpu;
mod network;
mod temperatures;

pub use battery::battery_task;
pub use cpu_memory::cpu_memory_process_task;
pub use disks::disks_task;
pub use global::global_metrics_task;
pub use gpu::gpu_task;
pub use network::network_task;
pub use temperatures::temperatures_task;

use super::metrics::*;

/// Updates sent from subsystem tasks to the orchestrator.
///
/// Each variant represents a complete update from one subsystem.
/// The orchestrator merges these updates into a complete SystemMetrics snapshot.
#[derive(Debug, Clone)]
pub enum SubsystemUpdate {
    /// CPU, Memory, and Process metrics (collected together as they share System)
    CpuMemoryProcess {
        cpu: CpuMetrics,
        memory: MemoryMetrics,
        processes: Vec<ProcessMetrics>,
    },

    /// GPU metrics (may be None if GPU not available or failed)
    Gpu(Option<GpuMetrics>),

    /// Disk metrics for all mounted filesystems
    Disks(Vec<DiskMetrics>),

    /// Network interface metrics
    Network(Vec<NetworkMetrics>),

    /// Temperature sensor readings
    Temperatures(Vec<TemperatureReading>),

    /// Battery and power source information
    Battery {
        power_source: PowerSource,
        battery_percent: Option<f32>,
        battery_time_remaining: Option<u32>,
    },

    /// Global system metrics (uptime, hostname, etc.)
    Global(GlobalMetrics),
}
