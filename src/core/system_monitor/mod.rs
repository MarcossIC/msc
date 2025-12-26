//! System monitoring core functionality.
//!
//! This module provides the business logic for collecting and processing
//! system metrics including CPU, memory, GPU, disk, and network information.

pub mod alerts;
mod collector;
pub mod disk_enrichment;
mod gpu;
mod history;
mod metrics;
pub mod process_tree;
pub mod runtime;
pub mod tasks;

pub use alerts::{evaluate_alerts, Alert, AlertCategory, AlertConfig, AlertSeverity};
pub use collector::{
    collect_battery_info, collect_cpu, collect_disks, collect_memory, collect_temperatures,
    sort_and_truncate_processes, CollectorConfig, MetricsCollector,
};
pub use gpu::GpuProvider;
pub use history::MetricsHistory;
pub use metrics::{
    BusType, CpuMetrics, DiskMetrics, DiskType, GlobalMetrics, GpuMetrics, GpuVendor,
    InterfaceSpeed, MemoryMetrics, NetworkMetrics, PowerSource, ProcessMetrics, SmartStatus,
    SystemMetrics, TemperatureReading,
};
pub use process_tree::{
    build_process_tree, flatten_tree, format_tree_indent, FlattenedProcess, ProcessTreeNode,
};
pub use runtime::{MetricsRuntime, UiState};
pub use tasks::SubsystemUpdate;
