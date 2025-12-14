use crate::core::system_info::types::NvidiaGpuMetrics;
use crate::error::Result;
use once_cell::sync::Lazy;

#[cfg(feature = "nvml")]
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
#[cfg(feature = "nvml")]
use nvml_wrapper::Nvml;

/// Singleton - NVML must be initialized ONCE only
///
/// NVML (NVIDIA Management Library) is the official NVIDIA library used by nvidia-smi.
/// It provides real-time access to GPU metrics without the overhead of spawning processes.
#[cfg(feature = "nvml")]
static NVML: Lazy<Option<Nvml>> = Lazy::new(|| Nvml::init().ok());

/// Get comprehensive NVIDIA GPU metrics using NVML (official NVIDIA library)
///
/// This function provides both static information (hardware specs) and dynamic
/// real-time metrics (temperature, clocks, utilization, etc.)
///
/// # Arguments
/// * `gpu_index` - Zero-based GPU index
///
/// # Returns
/// * `Ok(NvidiaGpuMetrics)` - Complete GPU metrics
/// * `Err(MscError)` - If NVML is not available or GPU index is invalid
///
/// # Example
/// ```
/// let metrics = get_nvidia_metrics_nvml(0)?;
/// println!("GPU: {}", metrics.name.unwrap_or_default());
/// println!("Temperature: {}°C", metrics.temperature_celsius.unwrap_or(0));
/// ```
#[cfg(feature = "nvml")]
pub fn get_nvidia_metrics_nvml(gpu_index: u32) -> Result<NvidiaGpuMetrics> {
    let nvml = NVML.as_ref().ok_or_else(|| {
        crate::error::MscError::other(
            "NVML not available (NVIDIA driver not installed or incompatible)",
        )
    })?;

    let device = nvml.device_by_index(gpu_index).map_err(|e| {
        crate::error::MscError::other(format!("Failed to get NVIDIA device {}: {}", gpu_index, e))
    })?;

    // === STATIC INFORMATION (Hardware specs) ===

    let name = device.name().ok();
    let uuid = device.uuid().ok();

    let pci_info = device.pci_info().ok();
    let pci_bus_id = pci_info.as_ref().map(|p| p.bus_id.clone());

    // CUDA Compute Capability (fundamental for architecture detection)
    let cuda_cc = device
        .cuda_compute_capability()
        .map(|cc| (cc.major as u32, cc.minor as u32))
        .ok();

    // Memory information
    let memory_info = device.memory_info().ok();
    let memory_total = memory_info.as_ref().map(|m| m.total);
    let memory_used = memory_info.as_ref().map(|m| m.used);
    let memory_free = memory_info.as_ref().map(|m| m.free);

    let memory_bus_width = device.memory_bus_width().ok();

    // CUDA Cores (real value from NVML, not estimated)
    let num_cores = device.num_cores().ok();

    // Calculate SM count from CUDA cores and compute capability
    let num_sm = match (cuda_cc, num_cores) {
        (Some(cc), Some(cores)) => Some(calculate_sm_count(cc, cores)),
        _ => None,
    };

    // PCIe information (maximum supported)
    let max_pcie_gen = device.max_pcie_link_gen().ok();
    let max_pcie_width = device.max_pcie_link_width().ok();

    // === DYNAMIC INFORMATION (Real-time metrics) ===

    let temperature = device.temperature(TemperatureSensor::Gpu).ok();

    let power_usage = device.power_usage().ok().map(|mw| mw as f32 / 1000.0); // milliwatts -> watts

    let power_limit = device.enforced_power_limit().ok().map(|mw| mw / 1000); // milliwatts -> watts

    let fan_speed = device.fan_speed(0).ok(); // Fan 0 (percentage)
    let fan_rpm = device.fan_speed_rpm(0).ok();

    // Clock frequencies (current)
    let clock_graphics = device.clock_info(Clock::Graphics).ok();
    let clock_memory = device.clock_info(Clock::Memory).ok();
    let clock_sm = device.clock_info(Clock::SM).ok();

    // GPU and memory utilization
    let utilization = device.utilization_rates().ok();
    let util_gpu = utilization.as_ref().map(|u| u.gpu);
    let util_mem = utilization.as_ref().map(|u| u.memory);

    // PCIe information (currently negotiated)
    let current_pcie_gen = device.current_pcie_link_gen().ok();
    let current_pcie_width = device.current_pcie_link_width().ok();

    // Performance state (P0 = max performance, P15 = idle)
    let pstate = device.performance_state().ok().map(|p| format!("{:?}", p));

    // Throttle reasons (thermal, power, etc.)
    let throttle = device.current_throttle_reasons().ok().and_then(|reasons| {
        if reasons.is_empty() {
            None
        } else {
            Some(vec![format!("{:?}", reasons)])
        }
    });

    // === ESTIMATED INFORMATION (Architecture-based) ===

    // Estimate Tensor Cores based on architecture
    let tensor_cores = cuda_cc.and_then(|cc| estimate_tensor_cores(cc, num_sm));

    // Estimate RT Cores based on architecture
    let rt_cores = cuda_cc.and_then(|cc| estimate_rt_cores(cc, num_sm));

    // Detect fabrication process node from architecture
    let process_nm = cuda_cc.and_then(detect_process_node);

    // TDP estimated from power limit
    let tdp_watts = power_limit;

    Ok(NvidiaGpuMetrics {
        // Static information
        name,
        uuid,
        pci_bus_id,
        cuda_compute_capability: cuda_cc,
        memory_total_bytes: memory_total,
        memory_bus_width_bits: memory_bus_width,
        cuda_cores: num_cores,
        sm_count: num_sm,
        max_pcie_generation: max_pcie_gen,
        max_pcie_lanes: max_pcie_width,

        // Static specs (estimated)
        tdp_watts,
        process_nm,
        tensor_cores,
        rt_cores,

        // Dynamic information
        temperature_celsius: temperature,
        power_draw_watts: power_usage,
        power_limit_watts: power_limit,
        fan_speed_percent: fan_speed,
        fan_rpm,
        clock_graphics_mhz: clock_graphics,
        clock_memory_mhz: clock_memory,
        clock_sm_mhz: clock_sm,
        utilization_gpu_percent: util_gpu,
        utilization_memory_percent: util_mem,
        memory_used_bytes: memory_used,
        memory_free_bytes: memory_free,
        current_pcie_generation: current_pcie_gen,
        current_pcie_lanes: current_pcie_width,
        pstate,
        throttle_reasons: throttle,
    })
}

/// Fallback when NVML feature is not enabled
#[cfg(not(feature = "nvml"))]
pub fn get_nvidia_metrics_nvml(_gpu_index: u32) -> Result<NvidiaGpuMetrics> {
    Err(crate::error::MscError::other(
        "NVML feature not enabled. Recompile with --features nvml",
    ))
}

/// Get GPU memory type using multiple strategies (NVML -> nvidia-smi -> fallback)
///
/// This function attempts to detect the GPU memory type using the following strategies:
/// 1. Query nvidia-smi for real memory type information (most reliable)
/// 2. Infer from NVML memory bus width and clock speed
/// 3. Fallback to architecture-based estimation
///
/// # Arguments
/// * `device` - Optional NVML device handle
///
/// # Returns
/// * `Some(String)` - Memory type (e.g., "GDDR7", "GDDR6X", "GDDR6")
/// * `None` - If detection fails
#[cfg(feature = "nvml")]
pub fn get_gpu_memory_type(device: Option<&nvml_wrapper::Device>) -> Option<String> {
    // Strategy 1: Try nvidia-smi (most accurate)
    if let Some(mem_type) = get_memory_type_from_nvidia_smi() {
        return Some(mem_type);
    }

    // Strategy 2: Infer from NVML device data
    if let Some(dev) = device {
        if let Some(mem_type) = infer_memory_type_from_nvml(dev) {
            return Some(mem_type);
        }
    }

    // Strategy 3: No reliable detection method available
    None
}

#[cfg(not(feature = "nvml"))]
pub fn get_gpu_memory_type(_device: Option<&()>) -> Option<String> {
    None
}

/// Query nvidia-smi for memory type information
///
/// nvidia-smi doesn't directly expose memory type, but we can infer it from
/// memory specifications (bandwidth, clock speed, bus width)
#[cfg(feature = "nvml")]
fn get_memory_type_from_nvidia_smi() -> Option<String> {
    use std::process::Command;

    // Query memory clock and bus width
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=clocks.max.memory,memory.bus",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next()?;
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

    if parts.len() >= 2 {
        let mem_clock: u32 = parts[0].parse().ok()?;
        let bus_width: u32 = parts[1].parse().ok()?;

        // Calculate effective data rate (MHz * 2 for DDR)
        let effective_rate = mem_clock * 2;

        // Infer memory type from effective rate and bus width
        // Modern GPUs typically use:
        // GDDR7: ~28-36 Gbps per pin
        // GDDR6X: ~19-24 Gbps per pin
        // GDDR6: ~14-16 Gbps per pin

        if effective_rate > 26000 {
            return Some("GDDR7".to_string());
        } else if effective_rate > 18000 && bus_width >= 256 {
            return Some("GDDR6X".to_string());
        } else if effective_rate > 12000 {
            return Some("GDDR6".to_string());
        }
    }

    None
}

/// Infer memory type from NVML device metrics
///
/// Uses memory clock speed and bus width to estimate memory type
#[cfg(feature = "nvml")]
fn infer_memory_type_from_nvml(device: &nvml_wrapper::Device) -> Option<String> {
    let bus_width = device.memory_bus_width().ok()?;
    let mem_clock = device.clock_info(Clock::Memory).ok()?;

    // Calculate effective data rate (DDR = double data rate)
    let effective_rate = mem_clock * 2;

    // Infer memory type based on effective transfer rate
    let mem_type = if effective_rate > 26000 {
        "GDDR7"
    } else if effective_rate > 18000 && bus_width >= 256 {
        "GDDR6X"
    } else if effective_rate > 12000 {
        "GDDR6"
    } else if effective_rate > 8000 {
        "GDDR5X"
    } else {
        "GDDR5"
    };

    Some(mem_type.to_string())
}

/// Calculate the number of Streaming Multiprocessors (SMs) based on compute capability and CUDA cores
///
/// Different GPU architectures have different numbers of CUDA cores per SM.
/// This function uses the official NVIDIA specifications.
///
/// # Arguments
/// * `cc` - CUDA Compute Capability (major, minor)
/// * `cuda_cores` - Total CUDA cores
///
/// # Returns
/// * Number of SMs
fn calculate_sm_count(cc: (u32, u32), cuda_cores: u32) -> u32 {
    if cuda_cores == 0 {
        return 0;
    }

    // Cores per SM by architecture (based on CUDA Compute Capability)
    let cores_per_sm = match cc {
        // Blackwell (RTX 50 series) - Compute Capability 10.x
        (10, _) => 128,
        // Ada Lovelace (RTX 40 series) - Compute Capability 8.9
        (8, 9) => 128,
        // Ampere (RTX 30 series consumer) - Compute Capability 8.6
        (8, 6) => 128,
        // Ampere (A100 datacenter) - Compute Capability 8.0
        (8, 0) => 64,
        // Turing (RTX 20 series, GTX 16 series) - Compute Capability 7.5
        (7, 5) => 64,
        // Volta (V100, Titan V) - Compute Capability 7.0, 7.2
        (7, 0) | (7, 2) => 64,
        // Pascal (GTX 10 series) - Compute Capability 6.x
        (6, _) => 64,
        // Maxwell (GTX 900/700 series) - Compute Capability 5.x
        (5, _) => 128,
        // Kepler (GTX 600 series) - Compute Capability 3.x
        (3, _) => 192,
        // Default fallback
        _ => 64,
    };

    cuda_cores / cores_per_sm
}

/// Estimate Tensor Cores count from compute capability and SM count
///
/// Tensor Cores are specialized AI/ML acceleration units introduced in Volta (CC 7.0).
/// Different architectures have different numbers of Tensor Cores per SM.
///
/// # Arguments
/// * `cc` - CUDA Compute Capability (major, minor)
/// * `sm_count` - Number of Streaming Multiprocessors
///
/// # Returns
/// * `Some(u32)` - Estimated Tensor Cores count
/// * `None` - If architecture doesn't have Tensor Cores
fn estimate_tensor_cores(cc: (u32, u32), sm_count: Option<u32>) -> Option<u32> {
    let sm = sm_count?;

    let tensor_per_sm = match cc {
        // Blackwell (RTX 50 series) - 5th gen Tensor Cores
        (10, _) => 4,
        // Ada Lovelace (RTX 40 series) - 4th gen Tensor Cores
        (8, 9) => 4,
        // Ampere GA10x (RTX 30 series) - 3rd gen Tensor Cores
        (8, 6) => 4,
        // Ampere A100 - 3rd gen Tensor Cores
        (8, 0) => 4,
        // Turing (RTX 20 series) - 2nd gen Tensor Cores
        (7, 5) => 8,
        // Volta (V100) - 1st gen Tensor Cores
        (7, 0) => 8,
        // GPUs before Volta don't have Tensor Cores
        _ => return None,
    };

    Some(sm * tensor_per_sm)
}

/// Estimate RT Cores count from compute capability and SM count
///
/// RT Cores are dedicated ray tracing acceleration units introduced in Turing (CC 7.5).
///
/// # Arguments
/// * `cc` - CUDA Compute Capability (major, minor)
/// * `sm_count` - Number of Streaming Multiprocessors
///
/// # Returns
/// * `Some(u32)` - Estimated RT Cores count
/// * `None` - If architecture doesn't have RT Cores
fn estimate_rt_cores(cc: (u32, u32), sm_count: Option<u32>) -> Option<u32> {
    let sm = sm_count?;

    let rt_per_sm = match cc {
        // Blackwell (RTX 50 series) - 4th gen RT Cores
        (10, _) => 1,
        // Ada Lovelace (RTX 40 series) - 3rd gen RT Cores
        (8, 9) => 1,
        // Ampere (RTX 30 series) - 2nd gen RT Cores
        (8, 6) => 1,
        // Turing (RTX 20 series) - 1st gen RT Cores
        (7, 5) => 1,
        // GPUs before Turing don't have RT Cores
        _ => return None,
    };

    Some(sm * rt_per_sm)
}

/// Detect fabrication process node from compute capability
///
/// Maps CUDA Compute Capability to the manufacturing process node (nm).
///
/// # Arguments
/// * `cc` - CUDA Compute Capability (major, minor)
///
/// # Returns
/// * `Some(u32)` - Process node in nanometers (e.g., 4nm, 7nm)
/// * `None` - If architecture is unknown
fn detect_process_node(cc: (u32, u32)) -> Option<u32> {
    match cc {
        // Blackwell - 4nm TSMC
        (10, _) => Some(4),
        // Ada Lovelace - 4nm TSMC (custom process)
        (8, 9) => Some(4),
        // Ampere - 8nm Samsung (GA10x) or 7nm TSMC (A100)
        (8, 6) => Some(8), // Consumer GPUs (Samsung 8nm)
        (8, 0) => Some(7), // Datacenter (TSMC 7nm)
        // Turing - 12nm TSMC
        (7, 5) => Some(12),
        // Volta - 12nm TSMC
        (7, 0) | (7, 2) => Some(12),
        // Pascal - 16nm TSMC
        (6, _) => Some(16),
        // Maxwell - 28nm
        (5, _) => Some(28),
        // Unknown architecture
        _ => None,
    }
}
