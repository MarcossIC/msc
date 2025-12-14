//! System monitoring core functionality.
//!
//! This module provides the business logic for collecting and processing
//! system metrics including CPU, memory, GPU, disk, and network information.

mod collector;
mod gpu;
mod history;
mod metrics;

pub use collector::{CollectorConfig, MetricsCollector};
pub use gpu::GpuProvider;
pub use history::MetricsHistory;
pub use metrics::{
    CpuMetrics, DiskMetrics, GpuMetrics, GpuVendor, MemoryMetrics, NetworkMetrics, ProcessMetrics,
    SystemMetrics, TemperatureReading,
};
