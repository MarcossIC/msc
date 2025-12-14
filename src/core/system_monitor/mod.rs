//! System monitoring core functionality.
//!
//! This module provides the business logic for collecting and processing
//! system metrics including CPU, memory, GPU, disk, and network information.

pub mod alerts;
mod collector;
mod gpu;
mod history;
mod metrics;
pub mod process_tree;

pub use alerts::{evaluate_alerts, Alert, AlertCategory, AlertConfig, AlertSeverity};
pub use collector::{CollectorConfig, MetricsCollector};
pub use gpu::GpuProvider;
pub use history::MetricsHistory;
pub use metrics::{
    CpuMetrics, DiskMetrics, GlobalMetrics, GpuMetrics, GpuVendor, MemoryMetrics, NetworkMetrics,
    PowerSource, ProcessMetrics, SystemMetrics, TemperatureReading,
};
pub use process_tree::{
    build_process_tree, flatten_tree, format_tree_indent, FlattenedProcess, ProcessTreeNode,
};
