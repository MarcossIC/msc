//! Alert system for monitoring critical conditions.
//!
//! Evaluates system metrics against configurable thresholds and generates alerts.

use super::metrics::SystemMetrics;
use serde::{Deserialize, Serialize};

/// Alert configuration with thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub cpu_warning: f32,     // Warning threshold (%)
    pub cpu_critical: f32,    // Critical threshold (%)
    pub memory_warning: f32,  // Warning threshold (%)
    pub memory_critical: f32, // Critical threshold (%)
    pub gpu_warning: f32,     // Warning threshold (%)
    pub gpu_critical: f32,    // Critical threshold (%)
    pub disk_warning: f32,    // Warning threshold (%)
    pub disk_critical: f32,   // Critical threshold (%)
    pub temp_warning: f32,    // Warning threshold (°C)
    pub temp_critical: f32,   // Critical threshold (°C)
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            cpu_warning: 75.0,
            cpu_critical: 90.0,
            memory_warning: 80.0,
            memory_critical: 95.0,
            gpu_warning: 80.0,
            gpu_critical: 95.0,
            disk_warning: 85.0,
            disk_critical: 95.0,
            temp_warning: 75.0,
            temp_critical: 90.0,
        }
    }
}

/// An individual alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub severity: AlertSeverity,
    pub category: AlertCategory,
    pub message: String,
    pub value: f32,
    pub threshold: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertCategory {
    CPU,
    Memory,
    GPU,
    Disk(String),        // Disk name
    Temperature(String), // Sensor name
    Network(String),     // Interface name
}

/// Evaluate system metrics and generate alerts
pub fn evaluate_alerts(metrics: &SystemMetrics, config: &AlertConfig) -> Vec<Alert> {
    let mut alerts = Vec::new();

    // CPU alerts
    if metrics.cpu.global_usage >= config.cpu_critical {
        alerts.push(Alert {
            severity: AlertSeverity::Critical,
            category: AlertCategory::CPU,
            message: format!(
                "CPU usage at {:.1}% (critical threshold: {:.1}%)",
                metrics.cpu.global_usage, config.cpu_critical
            ),
            value: metrics.cpu.global_usage,
            threshold: config.cpu_critical,
        });
    } else if metrics.cpu.global_usage >= config.cpu_warning {
        alerts.push(Alert {
            severity: AlertSeverity::Warning,
            category: AlertCategory::CPU,
            message: format!(
                "CPU usage at {:.1}% (warning threshold: {:.1}%)",
                metrics.cpu.global_usage, config.cpu_warning
            ),
            value: metrics.cpu.global_usage,
            threshold: config.cpu_warning,
        });
    }

    // Check for individual hot cores
    for (i, &usage) in metrics.cpu.per_core_usage.iter().enumerate() {
        if usage >= config.cpu_critical {
            alerts.push(Alert {
                severity: AlertSeverity::Critical,
                category: AlertCategory::CPU,
                message: format!("CPU Core {} at {:.1}% for extended period", i, usage),
                value: usage,
                threshold: config.cpu_critical,
            });
        }
    }

    // Memory alerts
    if metrics.memory.usage_percent >= config.memory_critical {
        alerts.push(Alert {
            severity: AlertSeverity::Critical,
            category: AlertCategory::Memory,
            message: format!(
                "Memory usage at {:.1}% (critical threshold: {:.1}%)",
                metrics.memory.usage_percent, config.memory_critical
            ),
            value: metrics.memory.usage_percent,
            threshold: config.memory_critical,
        });
    } else if metrics.memory.usage_percent >= config.memory_warning {
        alerts.push(Alert {
            severity: AlertSeverity::Warning,
            category: AlertCategory::Memory,
            message: format!(
                "Memory usage at {:.1}% (warning threshold: {:.1}%)",
                metrics.memory.usage_percent, config.memory_warning
            ),
            value: metrics.memory.usage_percent,
            threshold: config.memory_warning,
        });
    }

    // GPU alerts
    if let Some(ref gpu) = metrics.gpu {
        let gpu_usage = gpu.utilization_percent as f32;
        if gpu_usage >= config.gpu_critical {
            alerts.push(Alert {
                severity: AlertSeverity::Critical,
                category: AlertCategory::GPU,
                message: format!(
                    "GPU usage at {:.1}% (critical threshold: {:.1}%)",
                    gpu_usage, config.gpu_critical
                ),
                value: gpu_usage,
                threshold: config.gpu_critical,
            });
        } else if gpu_usage >= config.gpu_warning {
            alerts.push(Alert {
                severity: AlertSeverity::Warning,
                category: AlertCategory::GPU,
                message: format!(
                    "GPU usage at {:.1}% (warning threshold: {:.1}%)",
                    gpu_usage, config.gpu_warning
                ),
                value: gpu_usage,
                threshold: config.gpu_warning,
            });
        }

        // GPU Memory alert
        if gpu.memory_percent >= config.gpu_critical {
            alerts.push(Alert {
                severity: AlertSeverity::Critical,
                category: AlertCategory::GPU,
                message: format!(
                    "GPU memory at {:.1}% (critical threshold: {:.1}%)",
                    gpu.memory_percent, config.gpu_critical
                ),
                value: gpu.memory_percent,
                threshold: config.gpu_critical,
            });
        }
    }

    // Disk alerts
    for disk in &metrics.disks {
        if disk.usage_percent >= config.disk_critical {
            alerts.push(Alert {
                severity: AlertSeverity::Critical,
                category: AlertCategory::Disk(disk.mount_point.clone()),
                message: format!(
                    "Disk {} at {:.1}% capacity (critical threshold: {:.1}%)",
                    disk.mount_point, disk.usage_percent, config.disk_critical
                ),
                value: disk.usage_percent,
                threshold: config.disk_critical,
            });
        } else if disk.usage_percent >= config.disk_warning {
            alerts.push(Alert {
                severity: AlertSeverity::Warning,
                category: AlertCategory::Disk(disk.mount_point.clone()),
                message: format!(
                    "Disk {} at {:.1}% capacity (warning threshold: {:.1}%)",
                    disk.mount_point, disk.usage_percent, config.disk_warning
                ),
                value: disk.usage_percent,
                threshold: config.disk_warning,
            });
        }
    }

    // Temperature alerts
    for temp in &metrics.temperatures {
        if temp.current_celsius >= config.temp_critical {
            alerts.push(Alert {
                severity: AlertSeverity::Critical,
                category: AlertCategory::Temperature(temp.label.clone()),
                message: format!(
                    "{} at {:.1}°C (critical threshold: {:.1}°C)",
                    temp.label, temp.current_celsius, config.temp_critical
                ),
                value: temp.current_celsius,
                threshold: config.temp_critical,
            });
        } else if temp.current_celsius >= config.temp_warning {
            alerts.push(Alert {
                severity: AlertSeverity::Warning,
                category: AlertCategory::Temperature(temp.label.clone()),
                message: format!(
                    "{} at {:.1}°C (warning threshold: {:.1}°C)",
                    temp.label, temp.current_celsius, config.temp_warning
                ),
                value: temp.current_celsius,
                threshold: config.temp_warning,
            });
        }
    }

    // Network error alerts
    for net in &metrics.network {
        let total_errors = net.rx_errors + net.tx_errors;
        if total_errors > 0 {
            alerts.push(Alert {
                severity: if total_errors > 100 {
                    AlertSeverity::Warning
                } else {
                    AlertSeverity::Info
                },
                category: AlertCategory::Network(net.interface.clone()),
                message: format!("{} has {} network errors", net.interface, total_errors),
                value: total_errors as f32,
                threshold: 0.0,
            });
        }
    }

    alerts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::system_monitor::{CpuMetrics, MemoryMetrics};

    #[test]
    fn test_cpu_alert() {
        let config = AlertConfig::default();
        let metrics = SystemMetrics {
            cpu: CpuMetrics {
                global_usage: 91.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let alerts = evaluate_alerts(&metrics, &config);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);
    }

    #[test]
    fn test_memory_alert() {
        let config = AlertConfig::default();
        let metrics = SystemMetrics {
            memory: MemoryMetrics {
                usage_percent: 81.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let alerts = evaluate_alerts(&metrics, &config);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Warning);
    }

    #[test]
    fn test_no_alerts() {
        let config = AlertConfig::default();
        let metrics = SystemMetrics::default();

        let alerts = evaluate_alerts(&metrics, &config);
        assert_eq!(alerts.len(), 0);
    }
}
