#[cfg(feature = "nvml")]
use nvml_wrapper::{
    enum_wrappers::device::{Clock, TemperatureSensor},
    Device, Nvml,
};

use crate::core::system_monitor::{GpuMetrics, GpuProvider, GpuVendor};
use crate::error::{MscError, Result};

/// NVIDIA GPU provider using NVML
pub struct NvidiaGpuProvider {
    #[cfg(feature = "nvml")]
    nvml: Nvml,
    device_index: u32,
}

impl NvidiaGpuProvider {
    /// Create a new NVIDIA GPU provider
    ///
    /// Initializes NVML and selects the first available GPU.
    pub fn new() -> Result<Self> {
        Self::with_device_index(0)
    }

    /// Create provider for a specific GPU index
    pub fn with_device_index(index: u32) -> Result<Self> {
        #[cfg(feature = "nvml")]
        {
            let nvml = Nvml::init()
                .map_err(|e| MscError::gpu_not_available(format!("Failed to init NVML: {}", e)))?;

            // Verify device exists
            let _ = nvml.device_by_index(index).map_err(|e| {
                MscError::gpu_not_available(format!("GPU {} not found: {}", index, e))
            })?;

            Ok(Self {
                nvml,
                device_index: index,
            })
        }
        #[cfg(not(feature = "nvml"))]
        {
            Err(MscError::gpu_not_available(
                "NVIDIA GPU support not enabled",
            ))
        }
    }

    #[cfg(feature = "nvml")]
    fn get_device(&self) -> Result<Device<'_>> {
        self.nvml
            .device_by_index(self.device_index)
            .map_err(|e| MscError::metric_collection(format!("Failed to get GPU device: {}", e)))
    }
}

impl GpuProvider for NvidiaGpuProvider {
    fn vendor(&self) -> GpuVendor {
        GpuVendor::Nvidia
    }

    fn is_available(&self) -> bool {
        #[cfg(feature = "nvml")]
        {
            self.get_device().is_ok()
        }
        #[cfg(not(feature = "nvml"))]
        {
            false
        }
    }

    fn collect_metrics(&mut self) -> Result<GpuMetrics> {
        #[cfg(feature = "nvml")]
        {
            let device = self.get_device()?;

            let name = device
                .name()
                .unwrap_or_else(|_| "Unknown NVIDIA GPU".to_string());

            let utilization = device.utilization_rates().map(|u| u.gpu).unwrap_or(0);

            let memory_info = device.memory_info().map_err(|e| {
                MscError::metric_collection(format!("Failed to get memory info: {}", e))
            })?;

            let memory_percent = if memory_info.total > 0 {
                (memory_info.used as f32 / memory_info.total as f32) * 100.0
            } else {
                0.0
            };

            let temperature = device.temperature(TemperatureSensor::Gpu).ok();

            let fan_speed = device.fan_speed(0).ok();

            let power_draw = device
                .power_usage()
                .map(|p| p / 1000) // mW to W
                .ok();

            let power_limit = device
                .enforced_power_limit()
                .map(|p| p / 1000) // mW to W
                .ok();

            let clock_graphics = device.clock_info(Clock::Graphics).ok();
            let clock_memory = device.clock_info(Clock::Memory).ok();

            Ok(GpuMetrics {
                vendor: GpuVendor::Nvidia,
                name,
                utilization_percent: utilization,
                memory_used_bytes: memory_info.used,
                memory_total_bytes: memory_info.total,
                memory_percent,
                temperature_celsius: temperature,
                fan_speed_percent: fan_speed,
                power_draw_watts: power_draw,
                power_limit_watts: power_limit,
                clock_graphics_mhz: clock_graphics,
                clock_memory_mhz: clock_memory,
            })
        }
        #[cfg(not(feature = "nvml"))]
        {
            Err(MscError::gpu_not_available(
                "NVIDIA GPU support not enabled",
            ))
        }
    }
}
