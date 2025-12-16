use crate::core::system_monitor::{GpuMetrics, GpuProvider, GpuVendor};
use crate::error::{MscError, Result};

#[cfg(all(unix, feature = "rocm"))]
use rocm_smi_lib::{RocmSmi, DeviceHandle, TemperatureMetric, ClockType};

/// AMD GPU provider using ROCm SMI
pub struct AmdGpuProvider {
    #[cfg(all(unix, feature = "rocm"))]
    rocm: RocmSmi,
    #[allow(dead_code)]
    device_index: u32,
}

impl AmdGpuProvider {
    /// Create a new AMD GPU provider
    ///
    /// Initializes ROCm SMI and selects the first available GPU.
    pub fn new() -> Result<Self> {
        Self::with_device_index(0)
    }

    /// Create provider for a specific GPU index
    pub fn with_device_index(index: u32) -> Result<Self> {
        #[cfg(all(unix, feature = "rocm"))]
        {
            let rocm = RocmSmi::init()
                .map_err(|e| MscError::gpu_not_available(format!("Failed to init ROCm SMI: {:?}", e)))?;

            // Verify device exists
            let device_count = rocm.get_device_count()
                .map_err(|e| MscError::gpu_not_available(format!("Failed to get device count: {:?}", e)))?;

            if index >= device_count as u32 {
                return Err(MscError::gpu_not_available(format!(
                    "GPU {} not found (only {} devices available)",
                    index, device_count
                )));
            }

            Ok(Self {
                rocm,
                device_index: index,
            })
        }
        #[cfg(not(all(unix, feature = "rocm")))]
        {
            let _ = index; // Suppress unused warning
            Err(MscError::gpu_not_available(
                "AMD GPU support not enabled or not on Unix",
            ))
        }
    }

    #[cfg(all(unix, feature = "rocm"))]
    fn get_device(&self) -> Result<DeviceHandle> {
        self.rocm
            .get_device_handle(self.device_index)
            .map_err(|e| MscError::metric_collection(format!("Failed to get GPU device: {:?}", e)))
    }
}

impl GpuProvider for AmdGpuProvider {
    fn vendor(&self) -> GpuVendor {
        GpuVendor::Amd
    }

    fn is_available(&self) -> bool {
        #[cfg(all(unix, feature = "rocm"))]
        {
            self.get_device().is_ok()
        }
        #[cfg(not(all(unix, feature = "rocm")))]
        {
            false
        }
    }

    fn collect_metrics(&mut self) -> Result<GpuMetrics> {
        #[cfg(all(unix, feature = "rocm"))]
        {
            let device = self.get_device()?;

            // Get GPU name
            let name = self.rocm
                .get_device_name(&device)
                .unwrap_or_else(|_| "Unknown AMD GPU".to_string());

            // Get GPU utilization
            let utilization = self.rocm
                .get_busy_percent(&device)
                .unwrap_or(0) as u32;

            // Get memory info
            let memory_used = self.rocm
                .get_memory_used(&device)
                .unwrap_or(0);

            let memory_total = self.rocm
                .get_memory_total(&device)
                .unwrap_or(0);

            let memory_percent = if memory_total > 0 {
                (memory_used as f32 / memory_total as f32) * 100.0
            } else {
                0.0
            };

            // Get temperature
            let temperature = self.rocm
                .get_temperature(&device, TemperatureMetric::Edge)
                .ok()
                .map(|t| t as u32);

            // Get fan speed
            let fan_speed = self.rocm
                .get_fan_speed(&device, 0)
                .ok()
                .map(|f| f as u32);

            // Get power draw
            let power_draw = self.rocm
                .get_power_average(&device)
                .ok()
                .map(|p| (p / 1_000_000) as u32); // microwatts to watts

            // Get power cap/limit
            let power_limit = self.rocm
                .get_power_cap(&device)
                .ok()
                .map(|p| (p / 1_000_000) as u32); // microwatts to watts

            // Get clock frequencies
            let clock_graphics = self.rocm
                .get_clock(&device, ClockType::Sys)
                .ok()
                .map(|c| c as u32);

            let clock_memory = self.rocm
                .get_clock(&device, ClockType::Mem)
                .ok()
                .map(|c| c as u32);

            Ok(GpuMetrics {
                vendor: GpuVendor::Amd,
                name,
                utilization_percent: utilization,
                memory_used_bytes: memory_used,
                memory_total_bytes: memory_total,
                memory_percent,
                temperature_celsius: temperature,
                fan_speed_percent: fan_speed,
                power_draw_watts: power_draw,
                power_limit_watts: power_limit,
                clock_graphics_mhz: clock_graphics,
                clock_memory_mhz: clock_memory,
            })
        }
        #[cfg(not(all(unix, feature = "rocm")))]
        {
            Err(MscError::gpu_not_available(
                "AMD GPU support not enabled or not on Unix",
            ))
        }
    }
}
