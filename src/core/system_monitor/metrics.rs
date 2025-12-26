use serde::{Deserialize, Serialize};

/// Complete system metrics snapshot
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub timestamp: i64, // Unix timestamp
    pub global: GlobalMetrics,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub gpu: Option<GpuMetrics>,
    pub disks: Vec<DiskMetrics>,
    pub network: Vec<NetworkMetrics>,
    pub temperatures: Vec<TemperatureReading>,
    pub top_processes: Vec<ProcessMetrics>,
}

/// Global system-wide metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalMetrics {
    pub uptime_secs: u64,
    pub hostname: String,
    pub boot_time: i64, // Unix timestamp
    pub power_source: PowerSource,
    pub battery_percent: Option<f32>,
    pub battery_time_remaining_secs: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub enum PowerSource {
    Battery,
    #[default]
    AC,
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuMetrics {
    pub global_usage: f32,
    pub per_core_usage: Vec<f32>,
    pub frequencies_mhz: Vec<u64>,
    pub core_count: usize,
    pub brand: String,
    pub load_average: (f64, f64, f64), // 1, 5, 15 min
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,              // Real usage (excluding cache/buffers)
    pub cache_buffers_bytes: u64,     // Cache and buffers
    pub available_bytes: u64,
    pub usage_percent: f32,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuMetrics {
    pub vendor: GpuVendor,
    pub name: String,
    pub utilization_percent: u32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub memory_percent: f32,
    pub temperature_celsius: Option<u32>,
    pub fan_speed_percent: Option<u32>,
    pub power_draw_watts: Option<u32>,
    pub power_limit_watts: Option<u32>,
    pub clock_graphics_mhz: Option<u32>,
    pub clock_memory_mhz: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskMetrics {
    // Basic metrics (existing fields)
    pub name: String,
    pub mount_point: String,
    pub fs_type: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f32,
    pub read_bytes_per_sec: Option<u64>,
    pub write_bytes_per_sec: Option<u64>,

    // Extended metrics (new optional fields for enriched data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_type: Option<DiskType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bus_type: Option<BusType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface_speed: Option<InterfaceSpeed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smart_status: Option<SmartStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_celsius: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_on_hours: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes_read: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes_written: Option<u64>,
}

// Re-export types from system_info for convenience
pub use crate::core::system_info::types::{BusType, DiskType, InterfaceSpeed, SmartStatus};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub interface: String,
    pub rx_bytes_total: u64,
    pub tx_bytes_total: u64,
    pub rx_bytes_per_sec: u64,
    pub tx_bytes_per_sec: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_drops: u64,
    pub tx_drops: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemperatureReading {
    pub label: String,
    pub current_celsius: f32,
    pub max_celsius: f32,
    pub critical_celsius: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessMetrics {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub cpu_usage_percent: f32,
    pub memory_bytes: u64,
    pub memory_percent: f32,
    pub status: String,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
}
