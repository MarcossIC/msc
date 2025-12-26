use crate::core::system_info::types::*;
use colored::*;

/// Filter for controlling which system information sections to display
#[derive(Debug, Clone)]
pub struct DisplayFilter {
    pub cpu: bool,
    pub gpu: bool,
    pub memory: bool,
    pub motherboard: bool,
    pub network: bool,
    pub storage: bool,
    pub os: bool,
    pub npu: bool,
    pub energy: bool,
}

impl DisplayFilter {
    /// Returns a filter that shows all sections
    pub fn all() -> Self {
        Self {
            cpu: true,
            gpu: true,
            memory: true,
            motherboard: true,
            network: true,
            storage: true,
            os: true,
            npu: true,
            energy: true,
        }
    }
}

pub fn format_system_info(info: &SystemInfo, filter: &DisplayFilter) {
    println!("\n{}", "SYSTEM INFORMATION".bold().bright_cyan());
    println!("{}", "=".repeat(80));

    if filter.cpu {
        print_cpu_info(&info.cpu);
    }

    if filter.memory {
        print_memory_info(&info.memory);
    }

    if filter.gpu {
        print_gpu_info(&info.gpu);
    }

    if filter.motherboard {
        // Always show motherboard section if filter is enabled
        match &info.motherboard {
            Some(mb) => {
                print_motherboard_info(mb);
            }
            None => {}
        }
    }

    if filter.network {
        print_network_info(&info.network);
    }

    if filter.storage && !info.storage.is_empty() {
        print_storage_info(&info.storage);
    }

    if filter.os {
        print_os_info(&info.os);
    }

    if filter.npu {
        if let Some(ref npu) = info.npu {
            print_npu_info(npu);
        }
    }

    if filter.energy {
        // Energy section (combines battery and power plan)
        if info.battery.is_some() || info.power_plan.is_some() {
            print_energy_info(info.battery.as_ref(), info.power_plan.as_ref());
        }
    }

    println!();
}

fn print_section_header(title: &str) {
    println!("\n{}", title.bold().green());
    println!("{}", "-".repeat(title.len()));
}

fn print_cpu_info(cpu: &CpuInfo) {
    print_section_header("CPU");

    println!("  Model: {}", cpu.model);
    println!("  Vendor: {}", cpu.vendor);
    println!(
        "  Cores: {} physical, {} logical",
        cpu.physical_cores, cpu.logical_cores
    );

    // Detect microarchitecture
    let microarch = detect_cpu_microarchitecture(&cpu.model);
    let arch_str = if let Some(ref ma) = microarch {
        format!("{} ({})", cpu.architecture, ma)
    } else {
        cpu.architecture.clone()
    };
    println!("  Architecture: {}", arch_str);

    println!(
        "  Base Frequency: {:.2} GHz",
        cpu.frequency_mhz as f64 / 1000.0
    );

    if let Some(max_freq) = cpu.max_frequency_mhz {
        println!("  Max Frequency: {:.2} GHz", max_freq as f64 / 1000.0);
    }

    if let Some(turbo_freq) = cpu.turbo_frequency_mhz {
        println!("  Turbo Frequency: {:.2} GHz", turbo_freq as f64 / 1000.0);
    }

    if let Some(turbo_enabled) = cpu.turbo_boost_enabled {
        let status = if turbo_enabled {
            "Enabled".green()
        } else {
            "Disabled".red()
        };
        println!("  Turbo/Boost: {}", status);
    }

    // Cache information
    let mut cache_parts = Vec::new();
    if let Some(l1) = cpu.l1_cache_kb {
        cache_parts.push(format!("L1: {} KB", l1));
    }
    if let Some(l2) = cpu.l2_cache_kb {
        cache_parts.push(format!("L2: {} KB", l2));
    }
    if let Some(l3) = cpu.l3_cache_kb {
        cache_parts.push(format!("L3: {} KB", l3));
    }
    if !cache_parts.is_empty() {
        println!("  Cache: {}", cache_parts.join(", "));
    }

    // TDP information
    if let Some(tdp) = cpu.tdp_watts {
        if let Some(max_tdp) = cpu.max_tdp_watts {
            println!("  TDP: {} W (Max: {} W)", tdp, max_tdp);
        } else {
            println!("  TDP: {} W", tdp);
        }
    }

    // Current usage
    if let Some(usage) = cpu.current_usage_percent {
        let usage_str = if usage > 80.0 {
            format!("{:.1}%", usage).red()
        } else if usage > 50.0 {
            format!("{:.1}%", usage).yellow()
        } else {
            format!("{:.1}%", usage).green()
        };
        println!("  Current Usage: {}", usage_str);
    }

    // Instruction set support
    let mut instruction_parts = Vec::new();
    if cpu.instruction_sets.avx2 {
        instruction_parts.push("AVX2");
    }
    if cpu.instruction_sets.avx512 {
        instruction_parts.push("AVX-512");
    }
    if cpu.instruction_sets.fma3 {
        instruction_parts.push("FMA3");
    }
    if cpu.instruction_sets.sse4_2 {
        instruction_parts.push("SSE4.2");
    }
    if cpu.instruction_sets.sha {
        instruction_parts.push("SHA");
    }
    if cpu.instruction_sets.aes_ni {
        instruction_parts.push("AES-NI");
    }
    if !instruction_parts.is_empty() {
        println!(
            "  Instruction Sets: {}",
            instruction_parts.join(", ").green()
        );
    }

    // Topology information
    if let Some(numa) = cpu.numa_nodes {
        println!("  NUMA Nodes: {}", numa);
    }

    // AMD-specific topology
    if cpu.chiplet_count.is_some() || cpu.ccd_count.is_some() || cpu.ccx_count.is_some() {
        let mut topology_parts = Vec::new();
        if let Some(chiplets) = cpu.chiplet_count {
            topology_parts.push(format!(
                "{} Chiplet{}",
                chiplets,
                if chiplets > 1 { "s" } else { "" }
            ));
        }
        if let Some(ccds) = cpu.ccd_count {
            topology_parts.push(format!("{} CCD{}", ccds, if ccds > 1 { "s" } else { "" }));
        }
        if let Some(ccxs) = cpu.ccx_count {
            topology_parts.push(format!("{} CCX{}", ccxs, if ccxs > 1 { "es" } else { "" }));
        }
        if !topology_parts.is_empty() {
            println!("  AMD Topology: {}", topology_parts.join(", ").cyan());
        }
    }
}

fn print_memory_info(mem: &MemoryInfo) {
    print_section_header("Memory (RAM)");

    println!("  Total: {}", format_bytes(mem.total_bytes));
    println!(
        "  Available: {} ({:.1}%)",
        format_bytes(mem.available_bytes),
        (mem.available_bytes as f32 / mem.total_bytes as f32) * 100.0
    );
    println!(
        "  Used: {} ({:.1}%)",
        format_bytes(mem.used_bytes),
        mem.usage_percent
    );

    if let Some(ref ddr) = mem.ddr_type {
        println!("  Type: {}", ddr);
    }

    if let Some(speed) = mem.speed_mhz {
        println!("  Speed: {} MHz", speed);
    }

    if !mem.modules.is_empty() {
        let modules_str = format!(
            "{} x {}",
            mem.modules.len(),
            format_bytes(mem.modules[0].capacity_bytes)
        );
        println!("  Modules: {}", modules_str);
    }

    // Memory slots information
    if let (Some(total), Some(used)) = (mem.total_slots, mem.used_slots) {
        let available = total - used;
        let slots_str = if available > 0 {
            format!("{} used of {} ({} available)", used, total, available).yellow()
        } else {
            format!("{} used of {} (all occupied)", used, total).red()
        };
        println!("  Slots: {}", slots_str);
    }

    // Display memory prediction if available
    if let Some(ref prediction) = mem.prediction {
        println!();
        print_memory_prediction(prediction);
    }
}

fn print_memory_prediction(prediction: &MemoryPrediction) {
    println!("  {}", "Capacity Analysis:".cyan().bold());

    if prediction.minimum_guaranteed_bytes > 0 {
        println!(
            "    Minimum Guaranteed: {}",
            format_bytes(prediction.minimum_guaranteed_bytes).green()
        );
    }

    println!(
        "    Maximum Predicted: {}",
        format_bytes(prediction.maximum_predicted_bytes)
            .bright_green()
            .bold()
    );

    if let Some(smbios) = prediction.smbios_reported_bytes {
        let validation_mark = if prediction.is_smbios_validated {
            "✓".green()
        } else {
            "✗".red()
        };
        println!(
            "    SMBIOS Reported: {} {}",
            format_bytes(smbios),
            validation_mark
        );
    }

    if let Some(cpu_limit) = prediction.cpu_limit_bytes {
        println!("    CPU Limit: {}", format_bytes(cpu_limit));
    }

    let confidence_str = match prediction.confidence {
        ConfidenceLevel::High => "High".to_string().bright_green().bold(),
        ConfidenceLevel::Medium => "Medium".to_string().yellow(),
        ConfidenceLevel::Low => "Low".to_string().red(),
    };
    println!("    Confidence: {}", confidence_str);

    // Warnings
    if !prediction.warnings.is_empty() {
        println!();
        println!("    {}", "Warnings:".yellow().bold());
        for warning in &prediction.warnings {
            println!("      ! {}", warning.yellow());
        }
    }
}

fn print_gpu_info(gpus: &[GpuInfo]) {
    print_section_header("GPU");

    if gpus.is_empty() {
        println!("  No GPU detected");
        return;
    }

    for (i, gpu) in gpus.iter().enumerate() {
        if i > 0 {
            println!();
        }

        let gpu_type = if gpu.is_integrated {
            " (Integrated)"
        } else {
            ""
        };
        println!("  Model: {}{}", gpu.name, gpu_type);
        println!("  Vendor: {}", gpu.vendor);

        if let Some(vram) = gpu.vram_bytes {
            let vram_str = if let Some(ref mem_type) = gpu.memory_type {
                format!("{} {}", format_bytes(vram), mem_type)
            } else {
                format_bytes(vram)
            };
            println!("  VRAM: {}", vram_str);
        }

        if let Some(ref driver) = gpu.driver_version {
            println!("  Driver Version: {}", driver);
        }

        // Real-time metrics
        if let Some(temp) = gpu.temperature_celsius {
            let temp_str = if temp > 80 {
                format!("{}°C", temp).red()
            } else if temp > 70 {
                format!("{}°C", temp).yellow()
            } else {
                format!("{}°C", temp).green()
            };
            println!("  Temperature: {}", temp_str);
        }

        if let Some(core_clock) = gpu.core_clock_mhz {
            println!("  Core Clock: {} MHz", core_clock);
        }

        if let Some(mem_clock) = gpu.memory_clock_mhz {
            println!("  Memory Clock: {} MHz", mem_clock);
        }

        if let Some(power) = gpu.power_draw_watts {
            println!("  Power Draw: {:.1} W", power);
        }

        if let Some(fan_speed) = gpu.fan_speed_percent {
            println!("  Fan Speed: {}%", fan_speed);
        }

        // NVIDIA-specific metrics
        if let Some(ref nvidia) = gpu.nvidia_metrics {
            println!();
            println!("  {}", "NVIDIA Metrics:".cyan().bold());

            if let Some(tdp) = nvidia.tdp_watts {
                println!("    TDP: {} W", tdp);
            }

            if let Some(process) = nvidia.process_nm {
                println!("    Process: {} nm", process);
            }

            if let Some(boost) = nvidia.clock_sm_mhz {
                println!("    Boost Clock: {} MHz", boost);
            }

            if let Some(cuda) = nvidia.cuda_cores {
                println!("    CUDA Cores: {}", cuda);
            }

            if let Some(sm) = nvidia.sm_count {
                println!("    SM Count: {}", sm);
            }

            if let Some(tensor) = nvidia.tensor_cores {
                println!("    Tensor Cores: {}", tensor);
            }

            if let Some(rt) = nvidia.rt_cores {
                println!("    RT Cores: {}", rt);
            }

            if let (Some(gen), Some(lanes)) = (nvidia.max_pcie_generation, nvidia.max_pcie_lanes) {
                println!("    PCIe: Gen {} x{}", gen, lanes);
            }

            if let Some(fan_rpm) = nvidia.fan_rpm {
                println!("    Fan RPM: {}", fan_rpm);
            }
        }

        // AMD-specific metrics
        if let Some(ref amd) = gpu.amd_metrics {
            println!();
            println!("  {}", "AMD Metrics:".cyan().bold());

            if let Some(shaders) = amd.shader_count {
                println!("    Stream Processors: {}", shaders);
            }

            if let Some(cus) = amd.compute_units {
                println!("    Compute Units: {}", cus);
            }

            if let Some(rops) = amd.rop_count {
                println!("    ROPs: {}", rops);
            }

            if let Some(tmus) = amd.tmu_count {
                println!("    TMUs: {}", tmus);
            }

            if let Some(cache) = amd.infinity_cache_mb {
                println!("    Infinity Cache: {} MB", cache);
            }

            if let Some(process) = amd.process_nm {
                println!("    Process: {} nm", process);
            }

            if let Some(smartshift) = amd.smartshift_power_watts {
                println!("    SmartShift Power: {:.1} W", smartshift);
            }
        }
    }
}

fn print_motherboard_info(mb: &MotherboardInfo) {
    print_section_header("Motherboard");

    let mut has_data = false;

    if let Some(ref manufacturer) = mb.manufacturer {
        println!("  Manufacturer: {}", manufacturer);
        has_data = true;
    }

    if let Some(ref product) = mb.product {
        println!("  Model: {}", product);
        has_data = true;
    }

    if let Some(ref version) = mb.version {
        println!("  Revision: {}", version);
        has_data = true;
    }

    if let Some(ref chipset) = mb.chipset {
        println!("  Chipset: {}", chipset);
        has_data = true;
    }

    if let Some(ref bios_vendor) = mb.bios_vendor {
        println!("  BIOS Vendor: {}", bios_vendor);
        has_data = true;
    }

    if let Some(ref bios_version) = mb.bios_version {
        println!("  BIOS Version: {}", bios_version);
        has_data = true;
    }

    if let Some(ref tpm) = mb.tpm_version {
        println!("  TPM: {}", tpm);
        has_data = true;
    }

    if let Some(dimm_slots) = mb.dimm_slots {
        println!("  DIMM Slots: {}", dimm_slots);
        has_data = true;
    }

    if !has_data {
        println!("  {}", "No motherboard information available".dimmed());
    }

    // PCIe slots information
    if let Some(ref pcie_slots) = mb.pcie_slots {
        let occupied = pcie_slots.iter().filter(|s| s.occupied).count();
        let total = pcie_slots.len();
        let available = total - occupied;

        let pcie_summary = if available > 0 {
            format!(
                "{} occupied of {} ({} available)",
                occupied, total, available
            )
            .yellow()
        } else {
            format!("{} occupied of {} (all occupied)", occupied, total).red()
        };
        println!("  PCIe Slots: {}", pcie_summary);
    }

    // M.2 slots information
    if let (Some(total), Some(used)) = (mb.m2_slots_total, mb.m2_slots_used) {
        let available = total - used;
        let m2_str = if available > 0 {
            format!("{} used of {} ({} available)", used, total, available).yellow()
        } else {
            format!("{} used of {} (all occupied)", used, total).red()
        };
        println!("  M.2 Slots: {}", m2_str);
    }
}

fn print_network_info(net: &NetworkInfo) {
    print_section_header("Network");

    if !net.wifi_adapters.is_empty() {
        for wifi in &net.wifi_adapters {
            println!("  WiFi Adapter: {}", wifi.name.bold());
            println!("    Standard: {}", wifi.wifi_standard);

            if !wifi.bands.is_empty() {
                let bands_str = wifi
                    .bands
                    .iter()
                    .map(|b| b.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("    Bands: {}", bands_str);
            }

            // Current link speed
            if let Some(speed) = wifi.current_link_speed_mbps {
                let speed_str = if speed >= 1000 {
                    format!("{:.1} Gbps {}", speed as f32 / 1000.0, wifi.wifi_standard)
                } else {
                    format!("{} Mbps {}", speed, wifi.wifi_standard)
                };
                println!("    Link Speed: {}", speed_str.green());
            }

            // Chipset manufacturer
            if let Some(ref manufacturer) = wifi.chipset_manufacturer {
                println!("    Chipset: {}", manufacturer);
            }

            // IPv4 address
            if let Some(ref ipv4) = wifi.ipv4_address {
                println!("    IPv4: {}", ipv4.cyan());
            }

            // IPv6 address
            if let Some(ref ipv6) = wifi.ipv6_address {
                println!("    IPv6: {}", ipv6.cyan());
            }

            // Gateway latency
            if let Some(latency) = wifi.gateway_latency_ms {
                let latency_str = if latency < 10 {
                    format!("{} ms", latency).green()
                } else if latency < 50 {
                    format!("{} ms", latency).yellow()
                } else {
                    format!("{} ms", latency).red()
                };
                println!("    Gateway Latency: {}", latency_str);
            }
        }
    }

    if !net.ethernet_adapters.is_empty() {
        if !net.wifi_adapters.is_empty() {
            println!();
        }

        for eth in &net.ethernet_adapters {
            println!("  Ethernet: {}", eth.name.bold());

            if let Some(speed) = eth.speed_mbps {
                let speed_str = if speed >= 1000 {
                    format!("{:.1} Gbps", speed as f32 / 1000.0)
                } else {
                    format!("{} Mbps", speed)
                };
                println!("    Speed: {}", speed_str.green());
            }

            // IPv4 address
            if let Some(ref ipv4) = eth.ipv4_address {
                println!("    IPv4: {}", ipv4.cyan());
            }

            // IPv6 address
            if let Some(ref ipv6) = eth.ipv6_address {
                println!("    IPv6: {}", ipv6.cyan());
            }

            // Gateway latency
            if let Some(latency) = eth.gateway_latency_ms {
                let latency_str = if latency < 5 {
                    format!("{} ms", latency).green()
                } else if latency < 20 {
                    format!("{} ms", latency).yellow()
                } else {
                    format!("{} ms", latency).red()
                };
                println!("    Gateway Latency: {}", latency_str);
            }
        }
    }

    // Bluetooth adapters
    if !net.bluetooth_adapters.is_empty() {
        if !net.wifi_adapters.is_empty() || !net.ethernet_adapters.is_empty() {
            println!();
        }

        for bt in &net.bluetooth_adapters {
            let status_icon = if bt.is_enabled { "✓" } else { "✗" };
            let status_color = if bt.is_enabled {
                status_icon.green()
            } else {
                status_icon.red()
            };

            println!("  Bluetooth: {} {}", bt.name.bold(), status_color);
            println!("    Version: {}", bt.version);

            if let Some(ref manufacturer) = bt.manufacturer {
                println!("    Manufacturer: {}", manufacturer);
            }
        }
    }

    if net.wifi_adapters.is_empty()
        && net.ethernet_adapters.is_empty()
        && net.bluetooth_adapters.is_empty()
    {
        println!("  No network adapters detected");
    }
}

fn print_storage_info(storage: &[StorageInfo]) {
    use colored::Colorize;

    print_section_header("Storage");

    for (i, disk) in storage.iter().enumerate() {
        if i > 0 {
            println!();
        }

        // Build disk title with manufacturer and model if available
        let disk_title = if let (Some(ref mfg), Some(ref model)) = (&disk.manufacturer, &disk.model)
        {
            format!("{} {}", mfg, model)
        } else if let Some(ref model) = disk.model {
            model.clone()
        } else {
            disk.name.clone()
        };

        println!("  {} {}: {}", "Disk".cyan().bold(), i, disk_title.bold());

        // Technology type (HDD/SSD/NVMe)
        println!("    Type: {}", disk.disk_type.to_string().bold());

        // Interface (Bus type and speed)
        if let Some(ref bus_type) = disk.bus_type {
            if let Some(ref speed) = disk.interface_speed {
                println!(
                    "    Interface: {} - {}",
                    bus_type,
                    speed.to_string().green()
                );
            } else {
                println!("    Interface: {}", bus_type);
            }
        } else if let Some(ref speed) = disk.interface_speed {
            println!("    Interface: {}", speed.to_string().green());
        }

        // File System
        if !disk.file_system.is_empty() {
            println!("    File System: {}", disk.file_system);
        }

        // Mount point and capacity
        println!("    Mount Point: {}", disk.mount_point);
        println!("    Capacity: {}", format_bytes(disk.total_bytes));

        // Usage bar with percentage and free space
        println!(
            "    Usage: {} {:.1}% ({} free)",
            create_usage_bar(disk.usage_percent, 20),
            disk.usage_percent,
            format_bytes(disk.available_bytes)
        );

        // Temperature (if available)
        if let Some(temp) = disk.temperature_celsius {
            let temp_color = if temp >= 60 {
                temp.to_string().red()
            } else if temp >= 50 {
                temp.to_string().yellow()
            } else {
                temp.to_string().green()
            };
            println!("    Temperature: {}°C", temp_color);
        }

        // Health status (if available)
        if let Some(ref status) = disk.smart_status {
            let status_str = match status {
                crate::core::system_info::types::SmartStatus::Healthy => "Healthy".green(),
                crate::core::system_info::types::SmartStatus::Warning => "Warning".yellow(),
                crate::core::system_info::types::SmartStatus::Critical => "Critical".red(),
                crate::core::system_info::types::SmartStatus::Unknown => "Unknown".normal(),
            };
            println!("    Health: {}", status_str);
        }

        // Power-on hours (if available)
        if let Some(hours) = disk.power_on_hours {
            println!(
                "    Power-On Hours: {} hours ({:.1} days)",
                hours,
                hours as f64 / 24.0
            );
        }

        // Data written/read (if available)
        if let Some(written) = disk.total_bytes_written {
            println!("    Data Written: {}", format_bytes(written));
        }
        if let Some(read) = disk.total_bytes_read {
            println!("    Data Read: {}", format_bytes(read));
        }

        // Serial number and firmware (if available)
        if let Some(ref serial) = disk.serial_number {
            println!("    Serial: {}", serial);
        }
        if let Some(ref firmware) = disk.firmware_version {
            println!("    Firmware: {}", firmware);
        }
    }

    // Show expansion capacity
    #[cfg(windows)]
    {
        println!();
        if let Ok(slots) = crate::platform::system_info_windows::get_available_storage_slots() {
            println!("  {} ", "Expansion Capacity:".cyan().bold());

            // SATA ports
            if let (Some(total), Some(available)) = (slots.sata_total, slots.sata_available) {
                let status = if available > 0 {
                    format!("{} available", available).green()
                } else {
                    "full".yellow()
                };
                println!(
                    "    SATA Ports: {} used / {} total ({})",
                    slots.sata_used, total, status
                );

                if slots.sata_hot_swap {
                    println!("      Hot-Swap: Supported");
                }
            }

            // M.2 slots with detailed information
            if !slots.m2_slots.is_empty() {
                println!("    M.2 Slots:");
                for slot in &slots.m2_slots {
                    let status = if slot.is_used {
                        "Used".yellow()
                    } else {
                        "Available".green()
                    };

                    // Build slot type description
                    let mut slot_types = Vec::new();
                    if slot.supports_nvme {
                        slot_types.push("NVMe");
                    }
                    if slot.supports_sata {
                        slot_types.push("SATA");
                    }
                    let type_str = slot_types.join("/");

                    // Build interface description
                    let interface =
                        if let (Some(gen), Some(lanes)) = (slot.pcie_generation, slot.pcie_lanes) {
                            format!("PCIe {}.0 x{}", gen, lanes)
                        } else {
                            "Unknown".to_string()
                        };

                    // Build form factor description
                    let form_factor = if !slot.form_factors.is_empty() {
                        format!("({})", slot.form_factors.join(", "))
                    } else {
                        String::new()
                    };

                    println!(
                        "      Slot {}: {} {} {} - {}",
                        slot.slot_number, type_str, interface, form_factor, status
                    );
                }
            }
        }
    }
}

fn print_os_info(os: &OsInfo) {
    print_section_header("Operating System");

    println!("  Name: {}", os.name);
    println!("  Version: {}", os.version);

    if let Some(ref build) = os.build {
        println!("  Build: {}", build);
    }

    println!("  Architecture: {}", os.architecture);

    if let Some(ref kernel) = os.kernel_version {
        println!("  Kernel: {}", kernel);
    }
}

fn print_npu_info(npu: &NpuInfo) {
    print_section_header("NPU (Neural Processing Unit)");

    println!("  Name: {}", npu.name);

    if let Some(tops) = npu.tops {
        println!("  Performance: {:.1} TOPS", tops);
    }
}

fn print_energy_info(battery: Option<&BatteryInfo>, power_plan: Option<&PowerPlanInfo>) {
    use colored::Colorize;

    print_section_header("Energy");

    // Determine power source
    if let Some(battery) = battery {
        if battery.is_present {
            // Laptop with battery - show detailed battery information
            println!("  {} Battery", "Power Source:".bold());

            println!();
            println!("  {} ", "Battery Status".cyan().bold());

            // Battery state with color and icon
            let (state_str, _state_icon) = match battery.state {
                BatteryState::Charging => (battery.state.to_string().green(), "⚡"),
                BatteryState::Discharging => (battery.state.to_string().yellow(), "🔋"),
                BatteryState::Full => (battery.state.to_string().green(), "✓"),
                BatteryState::NotCharging => (battery.state.to_string().cyan(), "○"),
                BatteryState::Unknown => (battery.state.to_string().normal(), "?"),
            };
            print!("    Status: {}", state_str);

            // Charge percentage
            if let Some(percentage) = battery.percentage {
                let pct_str = if percentage >= 80 {
                    format!(" ({}%)", percentage).green()
                } else if percentage >= 20 {
                    format!(" ({}%)", percentage).yellow()
                } else {
                    format!(" ({}%)", percentage).red()
                };
                println!("{}", pct_str);
            } else {
                println!();
            }

            // Time remaining or time to full
            if let Some(time_secs) = battery.time_remaining_secs {
                if time_secs > 0 && battery.state == BatteryState::Discharging {
                    let hours = time_secs / 3600;
                    let minutes = (time_secs % 3600) / 60;
                    println!("    Time Remaining: {}h {}m", hours, minutes);
                }
            }

            if let Some(time_secs) = battery.time_to_full_secs {
                if time_secs > 0 && battery.state == BatteryState::Charging {
                    let hours = time_secs / 3600;
                    let minutes = (time_secs % 3600) / 60;
                    println!("    Time to Full Charge: {}h {}m", hours, minutes);
                }
            }

            // Power consumption/charging rate (REAL-TIME)
            if let Some(rate) = battery.discharge_rate_mw {
                if rate > 0 {
                    let watts = rate as f32 / 1000.0;
                    let power_str = if watts > 50.0 {
                        format!("{:.1} W", watts).red()
                    } else if watts > 20.0 {
                        format!("{:.1} W", watts).yellow()
                    } else {
                        format!("{:.1} W", watts).green()
                    };
                    println!("    Power Draw: {} (discharging)", power_str);
                } else if rate < 0 {
                    let watts = (-rate) as f32 / 1000.0;
                    println!("    Charging Power: {} W", format!("{:.1}", watts).green());
                }
            }

            // Voltage information
            if let Some(voltage) = battery.voltage_mv {
                let volts = voltage as f32 / 1000.0;
                print!("    Voltage: {:.2} V", volts);
                if let Some(design_voltage) = battery.design_voltage_mv {
                    let design_volts = design_voltage as f32 / 1000.0;
                    println!(" (nominal: {:.2} V)", design_volts);
                } else {
                    println!();
                }
            }

            println!();
            println!("  {} ", "Battery Specifications".cyan().bold());

            // Capacity in Wh (easier to understand than mWh)
            if let (Some(design_mwh), Some(current_mwh)) = (
                battery.design_capacity_mwh,
                battery.full_charge_capacity_mwh,
            ) {
                let design_wh = design_mwh as f32 / 1000.0;
                let current_wh = current_mwh as f32 / 1000.0;

                println!("    Design Capacity: {:.1} Wh", design_wh);
                println!(
                    "    Current Capacity: {:.1} Wh ({:.1}% of original)",
                    current_wh,
                    (current_wh / design_wh) * 100.0
                );
            }

            // Battery health and wear
            if let Some(health) = battery.health_percentage {
                let wear = 100 - health;
                let health_str = if health >= 90 {
                    format!("{}%", health).green()
                } else if health >= 70 {
                    format!("{}%", health).yellow()
                } else if health >= 50 {
                    format!("{}%", health).normal()
                } else {
                    format!("{}%", health).red()
                };

                let wear_str = if wear <= 10 {
                    format!("{}%", wear).green()
                } else if wear <= 30 {
                    format!("{}%", wear).yellow()
                } else {
                    format!("{}%", wear).red()
                };

                println!("    Battery Health: {} (wear: {})", health_str, wear_str);
            }

            // Cycle count with lifecycle estimation
            if let Some(cycles) = battery.cycle_count {
                let (cycles_str, lifecycle_str) = if cycles < 300 {
                    (format!("{}", cycles).green(), "Excellent".green())
                } else if cycles < 500 {
                    (format!("{}", cycles).yellow(), "Good".yellow())
                } else if cycles < 800 {
                    (format!("{}", cycles).normal(), "Fair".normal())
                } else {
                    (format!("{}", cycles).red(), "Consider replacement".red())
                };
                println!("    Cycle Count: {} cycles ({})", cycles_str, lifecycle_str);
            }

            // Technology
            if let Some(ref tech) = battery.technology {
                println!("    Technology: {}", tech);
            }

            // Manufacturer info
            if let Some(ref mfg) = battery.manufacturer {
                println!("    Manufacturer: {}", mfg);
            }

            if let Some(ref serial) = battery.serial_number {
                println!("    Serial Number: {}", serial);
            }

            if let Some(ref date) = battery.manufacture_date {
                println!("    Manufacture Date: {}", date);
            }
        } else {
            // Desktop PC
            println!("  {} AC Power (Mains)", "Power Source:".bold());
        }
    } else {
        // Desktop PC (no battery info available)
        println!("  {} AC Power (Mains)", "Power Source:".bold());
    }

    // Power plan (for both laptops and desktops)
    if let Some(power) = power_plan {
        println!();
        let plan_str = match power.active_plan {
            PowerPlan::HighPerformance => power.active_plan.to_string().green(),
            PowerPlan::UltimatePerformance => power.active_plan.to_string().bright_green(),
            PowerPlan::PowerSaver => power.active_plan.to_string().cyan(),
            PowerPlan::Balanced => power.active_plan.to_string().yellow(),
            PowerPlan::Custom(_) => power.active_plan.to_string().normal(),
        };
        println!("  Power Plan: {}", plan_str);

        if let Some(ref mode) = power.power_mode {
            let mode_str = match mode {
                PowerMode::BestPerformance => mode.to_string().bright_green(),
                PowerMode::BetterPerformance => mode.to_string().green(),
                PowerMode::BetterBattery => mode.to_string().yellow(),
                PowerMode::BestPowerEfficiency => mode.to_string().cyan(),
            };
            println!("  Power Mode: {}", mode_str);
        }

        // CPU states
        if let (Some(min_state), Some(max_state)) =
            (power.processor_min_state, power.processor_max_state)
        {
            println!("  CPU State: {}% min, {}% max", min_state, max_state);
        } else if let Some(min_state) = power.processor_min_state {
            println!("  CPU Min State: {}%", min_state);
        } else if let Some(max_state) = power.processor_max_state {
            println!("  CPU Max State: {}%", max_state);
        }

        // Display and sleep timeouts (show differently for laptop vs desktop)
        let has_battery = battery.map(|b| b.is_present).unwrap_or(false);

        if has_battery {
            // Laptop: show both AC and DC timeouts
            if let (Some(ac), Some(dc)) =
                (power.display_timeout_ac_secs, power.display_timeout_dc_secs)
            {
                let ac_str = format_timeout(ac);
                let dc_str = format_timeout(dc);
                println!("  Display Sleep: {} (AC), {} (Battery)", ac_str, dc_str);
            }

            if let (Some(ac), Some(dc)) = (power.sleep_timeout_ac_secs, power.sleep_timeout_dc_secs)
            {
                let ac_str = format_timeout(ac);
                let dc_str = format_timeout(dc);
                println!("  System Sleep: {} (AC), {} (Battery)", ac_str, dc_str);
            }
        } else {
            // Desktop: show only AC timeouts
            if let Some(timeout) = power.display_timeout_ac_secs {
                println!("  Display Sleep: {}", format_timeout(timeout));
            }

            if let Some(timeout) = power.sleep_timeout_ac_secs {
                println!("  System Sleep: {}", format_timeout(timeout));
            }
        }
    }
}

fn format_timeout(secs: u32) -> String {
    if secs == 0 {
        "Never".to_string()
    } else if secs < 60 {
        format!("{}s", secs)
    } else {
        format!("{}m", secs / 60)
    }
}

/// Create a usage bar with ASCII characters
fn create_usage_bar(usage_percent: f32, width: usize) -> String {
    use colored::Colorize;

    let filled = ((usage_percent / 100.0) * width as f32) as usize;
    let empty = width.saturating_sub(filled);

    let filled_char = "█";
    let empty_char = "░";

    let bar = format!("{}{}", filled_char.repeat(filled), empty_char.repeat(empty));

    // Color the bar based on usage
    let colored_bar = if usage_percent >= 85.0 {
        bar.red()
    } else if usage_percent >= 70.0 {
        bar.yellow()
    } else {
        bar.green()
    };

    format!("[{}]", colored_bar)
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn detect_cpu_microarchitecture(model: &str) -> Option<String> {
    let model_lower = model.to_lowercase();

    // AMD microarchitectures
    if model_lower.contains("ryzen") {
        // Ryzen AI 300 series (Zen 5)
        if model_lower.contains("ai 9")
            || model_lower.contains("ai 7")
            || model_lower.contains("ai 3")
        {
            return Some("Zen 5".to_string());
        }
        // Ryzen 9000 series (Zen 5)
        if model_lower.contains("9950")
            || model_lower.contains("9900")
            || model_lower.contains("9700")
            || model_lower.contains("9600")
        {
            return Some("Zen 5".to_string());
        }
        // Ryzen 7000 series (Zen 4)
        if model_lower.contains("7950")
            || model_lower.contains("7900")
            || model_lower.contains("7700")
            || model_lower.contains("7600")
            || model_lower.contains("7000")
        {
            return Some("Zen 4".to_string());
        }
        // Ryzen 5000 series (Zen 3)
        if model_lower.contains("5950")
            || model_lower.contains("5900")
            || model_lower.contains("5800")
            || model_lower.contains("5700")
            || model_lower.contains("5600")
            || model_lower.contains("5000")
        {
            return Some("Zen 3".to_string());
        }
        // Ryzen 3000 series (Zen 2)
        if model_lower.contains("3950")
            || model_lower.contains("3900")
            || model_lower.contains("3800")
            || model_lower.contains("3700")
            || model_lower.contains("3600")
            || model_lower.contains("3000")
        {
            return Some("Zen 2".to_string());
        }
        // Fallback for any Ryzen
        return Some("Zen".to_string());
    }

    // Intel microarchitectures
    if model_lower.contains("intel") {
        // Core Ultra (Arrow Lake / Lunar Lake)
        if model_lower.contains("core ultra")
            || model_lower.contains("ultra 9")
            || model_lower.contains("ultra 7")
            || model_lower.contains("ultra 5")
        {
            if model_lower.contains("200") {
                return Some("Arrow Lake".to_string());
            }
            return Some("Meteor Lake".to_string());
        }
        // 14th gen (Raptor Lake Refresh)
        if model_lower.contains("14900")
            || model_lower.contains("14700")
            || model_lower.contains("14600")
            || model_lower.contains("14400")
        {
            return Some("Raptor Lake".to_string());
        }
        // 13th gen (Raptor Lake)
        if model_lower.contains("13900")
            || model_lower.contains("13700")
            || model_lower.contains("13600")
            || model_lower.contains("13400")
        {
            return Some("Raptor Lake".to_string());
        }
        // 12th gen (Alder Lake)
        if model_lower.contains("12900")
            || model_lower.contains("12700")
            || model_lower.contains("12600")
            || model_lower.contains("12400")
            || model_lower.contains("12100")
        {
            return Some("Alder Lake".to_string());
        }
        // 11th gen (Rocket Lake)
        if model_lower.contains("11900")
            || model_lower.contains("11700")
            || model_lower.contains("11600")
            || model_lower.contains("11400")
        {
            return Some("Rocket Lake".to_string());
        }
    }

    None
}
