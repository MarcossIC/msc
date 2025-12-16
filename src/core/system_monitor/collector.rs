use std::time::Instant;

use sysinfo::{
    Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind,
    RefreshKind, System,
};

use crate::error::Result;
use crate::platform::gpu::get_gpu_provider;

use super::gpu::GpuProvider;
use super::metrics::*;

// ============================================================================
// PUBLIC PURE FUNCTIONS (for use by async tasks)
// ============================================================================

/// Collect CPU metrics from a System instance.
///
/// This is a pure function that can be called from async tasks.
pub fn collect_cpu(system: &System) -> CpuMetrics {
    let cpus = system.cpus();
    let load = System::load_average();

    CpuMetrics {
        global_usage: system.global_cpu_usage(),
        per_core_usage: cpus.iter().map(|cpu| cpu.cpu_usage()).collect(),
        frequencies_mhz: cpus.iter().map(|cpu| cpu.frequency()).collect(),
        core_count: cpus.len(),
        brand: cpus
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default(),
        load_average: (load.one, load.five, load.fifteen),
    }
}

/// Collect memory metrics from a System instance.
///
/// This is a pure function that can be called from async tasks.
pub fn collect_memory(system: &System) -> MemoryMetrics {
    let total = system.total_memory();
    let available = system.available_memory();
    let swap_total = system.total_swap();
    let swap_used = system.used_swap();

    // Calculate real used memory (excluding cache/buffers)
    // Real used = total - available
    let real_used = total.saturating_sub(available);

    // sysinfo's "used_memory" includes cache/buffers
    // cache_buffers = reported_used - real_used
    let reported_used = system.used_memory();
    let cache_buffers = reported_used.saturating_sub(real_used);

    MemoryMetrics {
        total_bytes: total,
        used_bytes: real_used,
        cache_buffers_bytes: cache_buffers,
        available_bytes: available,
        usage_percent: if total > 0 {
            (real_used as f32 / total as f32) * 100.0
        } else {
            0.0
        },
        swap_total_bytes: swap_total,
        swap_used_bytes: swap_used,
        swap_percent: if swap_total > 0 {
            (swap_used as f32 / swap_total as f32) * 100.0
        } else {
            0.0
        },
    }
}

/// Collect battery information.
///
/// This is a pure function that can be called from async tasks.
/// Returns (power_source, battery_percent, battery_time_remaining_secs)
pub fn collect_battery_info() -> (PowerSource, Option<f32>, Option<u32>) {
    // Try using the battery crate
    if let Ok(manager) = battery::Manager::new() {
        if let Ok(mut batteries) = manager.batteries() {
            if let Some(Ok(battery)) = batteries.next() {
                use battery::State;

                let power_source = match battery.state() {
                    State::Charging | State::Full => PowerSource::AC,
                    State::Discharging => PowerSource::Battery,
                    _ => PowerSource::Unknown,
                };

                let battery_percent = Some(battery.state_of_charge().value * 100.0);

                let battery_time = if battery.state() == State::Discharging {
                    battery.time_to_empty().map(|t| t.value as u32)
                } else {
                    None
                };

                return (power_source, battery_percent, battery_time);
            }
        }
    }

    // Fallback: No battery detected
    (PowerSource::AC, None, None)
}

/// Collect disk metrics from a Disks instance.
///
/// This is a pure function that can be called from async tasks.
pub fn collect_disks(disks: &Disks) -> Vec<DiskMetrics> {
    disks
        .iter()
        .map(|disk| {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total.saturating_sub(available);

            DiskMetrics {
                name: disk.name().to_string_lossy().to_string(),
                mount_point: disk.mount_point().to_string_lossy().to_string(),
                fs_type: disk.file_system().to_string_lossy().to_string(),
                total_bytes: total,
                available_bytes: available,
                usage_percent: if total > 0 {
                    (used as f32 / total as f32) * 100.0
                } else {
                    0.0
                },
                read_bytes_per_sec: None, // Requires separate tracking
                write_bytes_per_sec: None,
            }
        })
        .collect()
}

/// Collect temperature readings from a Components instance.
///
/// This is a pure function that can be called from async tasks.
pub fn collect_temperatures(components: &Components) -> Vec<TemperatureReading> {
    components
        .iter()
        .map(|comp| TemperatureReading {
            label: comp.label().to_string(),
            current_celsius: comp.temperature().unwrap_or(0.0),
            max_celsius: comp.max().unwrap_or(0.0),
            critical_celsius: comp.critical(),
        })
        .collect()
}

/// Sort and truncate processes by CPU usage.
///
/// This is a CPU-bound operation suitable for spawn_blocking.
pub fn sort_and_truncate_processes(
    processes: &std::collections::HashMap<sysinfo::Pid, sysinfo::Process>,
    total_memory: u64,
    top_n: usize,
) -> Vec<ProcessMetrics> {
    let mut procs: Vec<_> = processes
        .values()
        .map(|proc| {
            let mem = proc.memory();
            ProcessMetrics {
                pid: proc.pid().as_u32(),
                parent_pid: proc.parent().map(|p| p.as_u32()),
                name: proc.name().to_string_lossy().to_string(),
                cpu_usage_percent: proc.cpu_usage(),
                memory_bytes: mem,
                memory_percent: if total_memory > 0 {
                    (mem as f32 / total_memory as f32) * 100.0
                } else {
                    0.0
                },
                status: format!("{:?}", proc.status()),
                disk_read_bytes: proc.disk_usage().read_bytes,
                disk_write_bytes: proc.disk_usage().written_bytes,
            }
        })
        .collect();

    // Sort by CPU usage descending
    procs.sort_by(|a, b| {
        b.cpu_usage_percent
            .partial_cmp(&a.cpu_usage_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    procs.truncate(top_n);
    procs
}

// ============================================================================
// LEGACY METRICS COLLECTOR (will be deprecated)
// ============================================================================

/// Configuration for metrics collection
#[derive(Debug, Clone)]
pub struct CollectorConfig {
    pub collect_cpu: bool,
    pub collect_memory: bool,
    pub collect_gpu: bool,
    pub collect_disks: bool,
    pub collect_network: bool,
    pub collect_temperatures: bool,
    pub collect_processes: bool,
    pub top_processes_count: usize,
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            collect_cpu: true,
            collect_memory: true,
            collect_gpu: true,
            collect_disks: true,
            collect_network: true,
            collect_temperatures: true,
            collect_processes: true,
            top_processes_count: 10,
        }
    }
}

/// Collects system metrics from various sources
pub struct MetricsCollector {
    system: System,
    components: Components,
    disks: Disks,
    networks: Networks,
    gpu_provider: Option<Box<dyn GpuProvider>>,
    config: CollectorConfig,
    last_network_update: Option<Instant>,
    last_network_values: Vec<(u64, u64)>, // (rx, tx) per interface
}

impl MetricsCollector {
    /// Create a new MetricsCollector with default configuration
    pub fn new() -> Self {
        Self::with_config(CollectorConfig::default())
    }

    /// Create a new MetricsCollector with custom configuration
    pub fn with_config(config: CollectorConfig) -> Self {
        let refresh_kind = RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything())
            .with_processes(
                ProcessRefreshKind::nothing()
                    .with_cpu()
                    .with_memory()
                    .with_disk_usage(),
            );

        let system = System::new_with_specifics(refresh_kind);
        let components = Components::new_with_refreshed_list();
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();

        // Try to initialize GPU provider (graceful failure)
        let gpu_provider = if config.collect_gpu {
            get_gpu_provider().ok()
        } else {
            None
        };

        Self {
            system,
            components,
            disks,
            networks,
            gpu_provider,
            config,
            last_network_update: None,
            last_network_values: Vec::new(),
        }
    }

    /// Collect all configured metrics
    pub fn collect(&mut self) -> Result<SystemMetrics> {
        // Refresh system data
        self.system.refresh_all();

        let mut metrics = SystemMetrics {
            timestamp: chrono::Utc::now().timestamp(),
            global: self.collect_global(),
            ..Default::default()
        };

        if self.config.collect_cpu {
            metrics.cpu = self.collect_cpu();
        }

        if self.config.collect_memory {
            metrics.memory = self.collect_memory();
        }

        if self.config.collect_gpu {
            metrics.gpu = self.collect_gpu();
        }

        if self.config.collect_disks {
            self.disks.refresh(true);
            metrics.disks = self.collect_disks();
        }

        if self.config.collect_network {
            self.networks.refresh(true);
            metrics.network = self.collect_network();
        }

        if self.config.collect_temperatures {
            self.components.refresh(true);
            metrics.temperatures = self.collect_temperatures();
        }

        if self.config.collect_processes {
            metrics.top_processes = self.collect_top_processes();
        }

        Ok(metrics)
    }

    fn collect_global(&self) -> super::metrics::GlobalMetrics {
        use super::metrics::{GlobalMetrics};

        let boot_time = System::boot_time() as i64;
        let now = chrono::Utc::now().timestamp();
        let uptime_secs = (now - boot_time).max(0) as u64;

        let hostname = System::host_name().unwrap_or_else(|| "Unknown".to_string());

        // Try to get battery info
        let (power_source, battery_percent, battery_time_remaining) =
            self.collect_battery_info();

        GlobalMetrics {
            uptime_secs,
            hostname,
            boot_time,
            power_source,
            battery_percent,
            battery_time_remaining_secs: battery_time_remaining,
        }
    }

    fn collect_battery_info(&self) -> (PowerSource, Option<f32>, Option<u32>) {
        collect_battery_info()
    }

    fn collect_cpu(&self) -> CpuMetrics {
        collect_cpu(&self.system)
    }

    fn collect_memory(&self) -> MemoryMetrics {
        collect_memory(&self.system)
    }

    fn collect_gpu(&mut self) -> Option<GpuMetrics> {
        self.gpu_provider.as_mut()?.collect_metrics().ok()
    }

    fn collect_disks(&self) -> Vec<DiskMetrics> {
        collect_disks(&self.disks)
    }

    fn collect_network(&mut self) -> Vec<NetworkMetrics> {
        let now = Instant::now();
        let elapsed_secs = self
            .last_network_update
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(1.0);

        let current_values: Vec<_> = self
            .networks
            .values()
            .map(|data| (data.total_received(), data.total_transmitted()))
            .collect();

        let metrics: Vec<_> = self
            .networks
            .iter()
            .enumerate()
            .map(|(i, (name, data))| {
                let (prev_rx, prev_tx) = self
                    .last_network_values
                    .get(i)
                    .copied()
                    .unwrap_or((data.total_received(), data.total_transmitted()));

                let rx_diff = data.total_received().saturating_sub(prev_rx);
                let tx_diff = data.total_transmitted().saturating_sub(prev_tx);

                NetworkMetrics {
                    interface: name.to_string(),
                    rx_bytes_total: data.total_received(),
                    tx_bytes_total: data.total_transmitted(),
                    rx_bytes_per_sec: (rx_diff as f64 / elapsed_secs) as u64,
                    tx_bytes_per_sec: (tx_diff as f64 / elapsed_secs) as u64,
                    rx_packets: data.packets_received(),
                    tx_packets: data.packets_transmitted(),
                    rx_errors: data.errors_on_received(),
                    tx_errors: data.errors_on_transmitted(),
                    rx_drops: 0, // sysinfo doesn't provide drops directly
                    tx_drops: 0,
                }
            })
            .collect();

        self.last_network_update = Some(now);
        self.last_network_values = current_values;

        metrics
    }

    fn collect_temperatures(&self) -> Vec<TemperatureReading> {
        collect_temperatures(&self.components)
    }

    fn collect_top_processes(&self) -> Vec<ProcessMetrics> {
        let total_memory = self.system.total_memory();
        sort_and_truncate_processes(
            self.system.processes(),
            total_memory,
            self.config.top_processes_count,
        )
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
