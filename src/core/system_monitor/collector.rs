use std::time::Instant;

use sysinfo::{
    Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind,
    RefreshKind, System,
};

use crate::error::Result;
use crate::platform::gpu::get_gpu_provider;

use super::gpu::GpuProvider;
use super::metrics::*;

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

    fn collect_cpu(&self) -> CpuMetrics {
        let cpus = self.system.cpus();
        let load = System::load_average();

        CpuMetrics {
            global_usage: self.system.global_cpu_usage(),
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

    fn collect_memory(&self) -> MemoryMetrics {
        let total = self.system.total_memory();
        let used = self.system.used_memory();
        let swap_total = self.system.total_swap();
        let swap_used = self.system.used_swap();

        MemoryMetrics {
            total_bytes: total,
            used_bytes: used,
            available_bytes: self.system.available_memory(),
            usage_percent: if total > 0 {
                (used as f32 / total as f32) * 100.0
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

    fn collect_gpu(&mut self) -> Option<GpuMetrics> {
        self.gpu_provider.as_mut()?.collect_metrics().ok()
    }

    fn collect_disks(&self) -> Vec<DiskMetrics> {
        self.disks
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
                }
            })
            .collect();

        self.last_network_update = Some(now);
        self.last_network_values = current_values;

        metrics
    }

    fn collect_temperatures(&self) -> Vec<TemperatureReading> {
        self.components
            .iter()
            .map(|comp| TemperatureReading {
                label: comp.label().to_string(),
                current_celsius: comp.temperature().unwrap_or(0.0),
                max_celsius: comp.max().unwrap_or(0.0),
                critical_celsius: comp.critical(),
            })
            .collect()
    }

    fn collect_top_processes(&self) -> Vec<ProcessMetrics> {
        let total_memory = self.system.total_memory();
        let mut processes: Vec<_> = self
            .system
            .processes()
            .values()
            .map(|proc| {
                let mem = proc.memory();
                ProcessMetrics {
                    pid: proc.pid().as_u32(),
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
        processes.sort_by(|a, b| {
            b.cpu_usage_percent
                .partial_cmp(&a.cpu_usage_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        processes.truncate(self.config.top_processes_count);
        processes
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
