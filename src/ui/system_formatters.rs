use crate::core::system_info::types::*;
use colored::*;
use std::collections::HashMap;

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

/// One printable row inside a [`Section`].
enum Row {
    /// `label: value` pair. The label is right-padded (and dimmed) so that all
    /// fields sharing the same `indent` align their values vertically.
    Field {
        indent: usize,
        label: String,
        value: String,
    },
    /// A line printed verbatim — sub-headers, bars, bullets, anything that is
    /// not a plain `label: value` pair. Caller owns its indentation and color.
    Raw(String),
    /// A blank separator line.
    Blank,
}

/// Accumulates the rows of a single output section and renders them with
/// per-indent-level label alignment: within each indentation level the label
/// column is as wide as the longest label at that level, so values line up.
///
/// Why per-level and not per-section: a section may mix a 2-space group
/// (e.g. GPU `Model:`, `Vendor:`) with a 4-space sub-group (e.g. the
/// `NVIDIA Metrics:` fields). Each group aligns against its own peers.
struct Section {
    rows: Vec<Row>,
}

impl Section {
    fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Push an aligned `label: value` pair at the given indentation (in spaces).
    fn field(&mut self, indent: usize, label: &str, value: impl std::fmt::Display) {
        self.rows.push(Row::Field {
            indent,
            label: label.to_string(),
            value: value.to_string(),
        });
    }

    /// Push a verbatim line (already indented and colored by the caller).
    fn raw(&mut self, line: impl std::fmt::Display) {
        self.rows.push(Row::Raw(line.to_string()));
    }

    /// Push a blank separator line.
    fn blank(&mut self) {
        self.rows.push(Row::Blank);
    }

    fn render(&self) {
        // Longest "label:" length per indentation level.
        let mut widths: HashMap<usize, usize> = HashMap::new();
        for row in &self.rows {
            if let Row::Field { indent, label, .. } = row {
                let len = label.chars().count() + 1; // +1 for the ':'
                let entry = widths.entry(*indent).or_insert(0);
                if len > *entry {
                    *entry = len;
                }
            }
        }

        for row in &self.rows {
            match row {
                Row::Blank => println!(),
                Row::Raw(line) => println!("{}", line),
                Row::Field {
                    indent,
                    label,
                    value,
                } => {
                    let width = widths[indent];
                    // Pad BEFORE dimming: `.dimmed()` injects ANSI codes that
                    // would corrupt the width count.
                    let label_colon = format!("{}:", label);
                    let padded = format!("{:<w$}", label_colon, w = width);
                    println!("{}{}  {}", " ".repeat(*indent), padded.dimmed(), value);
                }
            }
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
        if let Some(mb) = &info.motherboard {
            print_motherboard_info(mb);
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

    let mut s = Section::new();

    s.field(2, "Model", &cpu.model);
    s.field(2, "Vendor", &cpu.vendor);
    s.field(
        2,
        "Cores",
        format!("{} physical, {} logical", cpu.physical_cores, cpu.logical_cores),
    );

    // Microarchitecture. For AMD the CPUID family is GROUND TRUTH (family 0x1A ≡
    // Zen 5, by AMD's definition) — used as the primary source, with the marketing
    // string heuristic as fallback. Intel reports every Core generation as family
    // 6, so the family is useless there and we keep the string heuristic. We then
    // append the raw CPUID family/model in hex (matching tools like MSI Afterburner,
    // e.g. "Family 1Ah Model 60h") to pin the exact silicon.
    let is_amd = cpu.vendor.eq_ignore_ascii_case("AuthenticAMD");
    let microarch = cpu
        .cpu_family
        .filter(|_| is_amd)
        .and_then(amd_microarch_from_family)
        .or_else(|| detect_cpu_microarchitecture(&cpu.model));

    let mut arch_parts: Vec<String> = Vec::new();
    if let Some(ma) = microarch {
        arch_parts.push(ma);
    }
    if let Some(family) = cpu.cpu_family {
        arch_parts.push(format!("Family {:X}h", family));
    }
    if let Some(model) = cpu.cpu_model {
        arch_parts.push(format!("Model {:X}h", model));
    }
    let arch_str = if arch_parts.is_empty() {
        cpu.architecture.clone()
    } else {
        format!("{} ({})", cpu.architecture, arch_parts.join(" · "))
    };
    s.field(2, "Architecture", arch_str);

    // Silicon stepping. Only shown when NON-zero: a stepping of 0 (the A0 revision)
    // carries no signal on its own, so we treat it as "nothing" and omit the line.
    if let Some(stepping) = cpu.cpu_stepping.filter(|&s| s != 0) {
        s.field(2, "Stepping", stepping.to_string());
    }

    // Loaded microcode revision — the value HWiNFO labels "MCU", read driver-less
    // from the registry. Hex to match how every tool prints it (e.g. 0xB600032).
    if let Some(microcode) = cpu.cpu_microcode {
        s.field(2, "Microcode", format!("0x{:X}", microcode));
    }

    s.field(
        2,
        "Base Frequency",
        format!("{:.2} GHz", cpu.frequency_mhz as f64 / 1000.0),
    );

    if let Some(max_freq) = cpu.max_frequency_mhz {
        s.field(2, "Max Frequency", format!("{:.2} GHz", max_freq as f64 / 1000.0));
    }

    if let Some(turbo_freq) = cpu.turbo_frequency_mhz {
        s.field(
            2,
            "Turbo Frequency",
            format!("{:.2} GHz", turbo_freq as f64 / 1000.0),
        );
    }

    if let Some(turbo_enabled) = cpu.turbo_boost_enabled {
        let status = if turbo_enabled {
            "Enabled".green()
        } else {
            "Disabled".red()
        };
        s.field(2, "Turbo/Boost", status);
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
        s.field(2, "Cache", cache_parts.join(", "));
    }

    // TDP information
    if let Some(tdp) = cpu.tdp_watts {
        if let Some(max_tdp) = cpu.max_tdp_watts {
            s.field(2, "TDP", format!("{} W (Max: {} W)", tdp, max_tdp));
        } else {
            s.field(2, "TDP", format!("{} W", tdp));
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
        s.field(2, "Current Usage", usage_str);
    }

    // Temperature (ACPI thermal zone — laptops run hotter than GPUs, so the
    // thresholds sit higher than the GPU section's). Always shown so the user
    // sees the field was probed; "-" marks the rare machine where no driver-less
    // sensor is exposed at all.
    let temp_str = match cpu.temperature_celsius {
        Some(temp) if temp >= 90 => format!("{}°C", temp).red(),
        Some(temp) if temp >= 75 => format!("{}°C", temp).yellow(),
        Some(temp) => format!("{}°C", temp).green(),
        None => "-".dimmed(),
    };
    s.field(2, "Temperature", temp_str);

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
        s.field(2, "Instruction Sets", instruction_parts.join(", ").green());
    }

    // Virtualization — two DISTINCT signals, never conflated:
    if let Some(ref virt) = cpu.virtualization {
        // 1) Firmware VT-x/AMD-V toggle: the actionable "can I run VMs" signal.
        match virt.firmware_enabled {
            Some(true) => s.field(2, "Virtualization", "Enabled (VT-x/AMD-V)".green()),
            Some(false) => {
                s.field(2, "Virtualization", "Disabled in firmware (VT-x/AMD-V)".yellow())
            }
            None => {} // WMI didn't expose it — omit, never guess.
        }
        // 2) Hypervisor-present bit. Honest: "active" ≠ "this is a VM" — Hyper-V,
        // WSL2 and VBS/Memory Integrity set it on a normal Windows 11 host too.
        if virt.hypervisor_present {
            s.field(2, "Hypervisor", "Active (Hyper-V/VBS/WSL2 or VM)".cyan());
        }
    }

    // Topology information
    if let Some(numa) = cpu.numa_nodes {
        s.field(2, "NUMA Nodes", numa);
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
            s.field(2, "AMD Topology", topology_parts.join(", ").cyan());
        }
    }

    s.render();
}

fn print_memory_info(mem: &MemoryInfo) {
    print_section_header("Memory (RAM)");

    let mut s = Section::new();

    s.field(2, "Total", format_bytes(mem.total_bytes));
    s.field(
        2,
        "Available",
        format!(
            "{} ({:.1}%)",
            format_bytes(mem.available_bytes),
            (mem.available_bytes as f32 / mem.total_bytes as f32) * 100.0
        ),
    );
    s.field(
        2,
        "Used",
        format!("{} ({:.1}%)", format_bytes(mem.used_bytes), mem.usage_percent),
    );

    if let Some(ref ddr) = mem.ddr_type {
        s.field(2, "Type", ddr);
    }

    if let Some(speed) = mem.speed_mhz {
        s.field(2, "Speed", format!("{} MHz", speed));
    }

    if !mem.modules.is_empty() {
        s.field(
            2,
            "Modules",
            format!(
                "{} x {}",
                mem.modules.len(),
                format_bytes(mem.modules[0].capacity_bytes)
            ),
        );
    }

    // Memory slots information
    if let (Some(total), Some(used)) = (mem.total_slots, mem.used_slots) {
        let available = total - used;
        let slots_str = if available > 0 {
            format!("{} used of {} ({} available)", used, total, available).yellow()
        } else {
            format!("{} used of {} (all occupied)", used, total).red()
        };
        s.field(2, "Slots", slots_str);
    }

    s.render();

    // Display memory prediction if available
    if let Some(ref prediction) = mem.prediction {
        println!();
        print_memory_prediction(prediction);
    }
}

fn print_memory_prediction(prediction: &MemoryPrediction) {
    let mut s = Section::new();

    s.raw(format!("  {}", "Capacity Analysis:".cyan().bold()));

    if prediction.minimum_guaranteed_bytes > 0 {
        s.field(
            4,
            "Minimum Guaranteed",
            format_bytes(prediction.minimum_guaranteed_bytes).green(),
        );
    }

    s.field(
        4,
        "Maximum Predicted",
        format_bytes(prediction.maximum_predicted_bytes)
            .bright_green()
            .bold(),
    );

    if let Some(smbios) = prediction.smbios_reported_bytes {
        let validation_mark = if prediction.is_smbios_validated {
            "✓".green()
        } else {
            "✗".red()
        };
        s.field(
            4,
            "SMBIOS Reported",
            format!("{} {}", format_bytes(smbios), validation_mark),
        );
    }

    if let Some(cpu_limit) = prediction.cpu_limit_bytes {
        s.field(4, "CPU Limit", format_bytes(cpu_limit));
    }

    let confidence_str = match prediction.confidence {
        ConfidenceLevel::High => "High".to_string().bright_green().bold(),
        ConfidenceLevel::Medium => "Medium".to_string().yellow(),
        ConfidenceLevel::Low => "Low".to_string().red(),
    };
    s.field(4, "Confidence", confidence_str);

    // Warnings
    if !prediction.warnings.is_empty() {
        s.blank();
        s.raw(format!("    {}", "Warnings:".yellow().bold()));
        for warning in &prediction.warnings {
            s.raw(format!("      ! {}", warning.yellow()));
        }
    }

    s.render();
}

fn print_gpu_info(gpus: &[GpuInfo]) {
    print_section_header("GPU");

    if gpus.is_empty() {
        println!("  No GPU detected");
        return;
    }

    let mut s = Section::new();

    for (i, gpu) in gpus.iter().enumerate() {
        if i > 0 {
            s.blank();
        }

        let gpu_type = if gpu.is_integrated {
            " (Integrated)"
        } else {
            ""
        };
        s.field(2, "Model", format!("{}{}", gpu.name, gpu_type));
        s.field(2, "Vendor", &gpu.vendor);

        // Board assembler / laptop OEM (ASUS, MSI, Acer…), distinct from the
        // chip vendor above. Only shown when we could parse it from the PNP ID.
        if let Some(ref subsystem) = gpu.subsystem_vendor {
            s.field(2, "Subsystem", subsystem);
        }

        if let Some(vram) = gpu.vram_bytes {
            let mut vram_str = if let Some(ref mem_type) = gpu.memory_type {
                format!("{} {}", format_bytes(vram), mem_type)
            } else {
                format_bytes(vram)
            };
            // Integrated GPUs report ONLY the fixed UMA frame buffer (the size
            // the firmware carves out in BIOS) via AdapterRAM. They additionally
            // borrow system RAM on demand, so this number is NOT a hard ceiling.
            // Label it honestly without fabricating the dynamic-share figure.
            if gpu.is_integrated {
                vram_str = format!("{} dedicated {}", vram_str, "+ Dynamic Shared".dimmed());
            }
            s.field(2, "VRAM", vram_str);
        }

        if let Some(ref driver) = gpu.driver_version {
            s.field(2, "Driver Version", driver);
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
            s.field(2, "Temperature", temp_str);
        }

        if let Some(core_clock) = gpu.core_clock_mhz {
            s.field(2, "Core Clock", format!("{} MHz", core_clock));
        }

        if let Some(mem_clock) = gpu.memory_clock_mhz {
            s.field(2, "Memory Clock", format!("{} MHz", mem_clock));
        }

        if let Some(power) = gpu.power_draw_watts {
            s.field(2, "Power Draw", format!("{:.1} W", power));
        }

        if let Some(fan_speed) = gpu.fan_speed_percent {
            s.field(2, "Fan Speed", format!("{}%", fan_speed));
        }

        // NVIDIA-specific metrics
        if let Some(ref nvidia) = gpu.nvidia_metrics {
            s.blank();
            s.raw(format!("  {}", "NVIDIA Metrics:".cyan().bold()));

            if let Some(tdp) = nvidia.tdp_watts {
                s.field(4, "TDP", format!("{} W", tdp));
            }

            if let Some(process) = nvidia.process_nm {
                s.field(4, "Process", format!("{} nm", process));
            }

            if let Some(boost) = nvidia.clock_sm_mhz {
                s.field(4, "Boost Clock", format!("{} MHz", boost));
            }

            if let Some(cuda) = nvidia.cuda_cores {
                s.field(4, "CUDA Cores", cuda);
            }

            if let Some(sm) = nvidia.sm_count {
                s.field(4, "SM Count", sm);
            }

            if let Some(tensor) = nvidia.tensor_cores {
                s.field(4, "Tensor Cores", tensor);
            }

            if let Some(rt) = nvidia.rt_cores {
                s.field(4, "RT Cores", rt);
            }

            if let (Some(gen), Some(lanes)) = (nvidia.max_pcie_generation, nvidia.max_pcie_lanes) {
                s.field(4, "PCIe", format!("Gen {} x{}", gen, lanes));
            }

            if let Some(fan_rpm) = nvidia.fan_rpm {
                s.field(4, "Fan RPM", fan_rpm);
            }
        }

        // AMD-specific metrics
        if let Some(ref amd) = gpu.amd_metrics {
            s.blank();
            s.raw(format!("  {}", "AMD Metrics:".cyan().bold()));

            if let Some(shaders) = amd.shader_count {
                s.field(4, "Stream Processors", shaders);
            }

            if let Some(cus) = amd.compute_units {
                s.field(4, "Compute Units", cus);
            }

            if let Some(rops) = amd.rop_count {
                s.field(4, "ROPs", rops);
            }

            if let Some(tmus) = amd.tmu_count {
                s.field(4, "TMUs", tmus);
            }

            if let Some(cache) = amd.infinity_cache_mb {
                s.field(4, "Infinity Cache", format!("{} MB", cache));
            }

            if let Some(process) = amd.process_nm {
                s.field(4, "Process", format!("{} nm", process));
            }

            if let Some(smartshift) = amd.smartshift_power_watts {
                s.field(4, "SmartShift Power", format!("{:.1} W", smartshift));
            }
        }
    }

    s.render();
}

fn print_motherboard_info(mb: &MotherboardInfo) {
    print_section_header("Motherboard");

    let mut s = Section::new();
    let mut has_data = false;

    if let Some(ref manufacturer) = mb.manufacturer {
        s.field(2, "Manufacturer", manufacturer);
        has_data = true;
    }

    if let Some(ref product) = mb.product {
        s.field(2, "Model", product);
        has_data = true;
    }

    if let Some(ref version) = mb.version {
        s.field(2, "Revision", version);
        has_data = true;
    }

    if let Some(ref chipset) = mb.chipset {
        s.field(2, "Chipset", chipset);
        has_data = true;
    }

    if let Some(ref bios_vendor) = mb.bios_vendor {
        s.field(2, "BIOS Vendor", bios_vendor);
        has_data = true;
    }

    if let Some(ref bios_version) = mb.bios_version {
        s.field(2, "BIOS Version", bios_version);
        has_data = true;
    }

    if let Some(ref bios_date) = mb.bios_date {
        s.field(2, "BIOS Date", bios_date);
        has_data = true;
    }

    // Boot firmware mode (UEFI vs legacy BIOS) — driver-less via GetFirmwareType.
    if let Some(ref firmware_mode) = mb.firmware_mode {
        s.field(2, "Firmware", firmware_mode);
        has_data = true;
    }

    if let Some(ref tpm) = mb.tpm_version {
        s.field(2, "TPM", tpm);
        has_data = true;
    }

    // Secure Boot. Enabled = green; Disabled = red (a real security downgrade);
    // "Not supported" (legacy BIOS) is neutral/dimmed — it's a fact, not a fault.
    if let Some(secure_boot) = mb.secure_boot {
        use crate::core::system_info::types::SecureBootStatus;
        let sb_str = match secure_boot {
            SecureBootStatus::Enabled => secure_boot.to_string().green(),
            SecureBootStatus::Disabled => secure_boot.to_string().red(),
            SecureBootStatus::Unsupported => secure_boot.to_string().dimmed(),
        };
        s.field(2, "Secure Boot", sb_str);
        has_data = true;
    }

    if let Some(dimm_slots) = mb.dimm_slots {
        s.field(2, "DIMM Slots", dimm_slots);
        has_data = true;
    }

    if !has_data {
        s.raw(format!("  {}", "No motherboard information available".dimmed()));
    }

    // PCIe slots information
    if let Some(ref pcie_slots) = mb.pcie_slots {
        let occupied = pcie_slots.iter().filter(|s| s.occupied).count();
        let total = pcie_slots.len();
        let available = total - occupied;

        let pcie_summary = if available > 0 {
            format!("{} occupied of {} ({} available)", occupied, total, available).yellow()
        } else {
            format!("{} occupied of {} (all occupied)", occupied, total).red()
        };
        s.field(2, "PCIe Slots", pcie_summary);
    }

    // M.2 slots information
    if let (Some(total), Some(used)) = (mb.m2_slots_total, mb.m2_slots_used) {
        let available = total - used;
        let m2_str = if available > 0 {
            format!("{} used of {} ({} available)", used, total, available).yellow()
        } else {
            format!("{} used of {} (all occupied)", used, total).red()
        };
        s.field(2, "M.2 Slots", m2_str);
    }

    s.render();
}

fn print_network_info(net: &NetworkInfo) {
    print_section_header("Network");

    let mut s = Section::new();

    if !net.wifi_adapters.is_empty() {
        for wifi in &net.wifi_adapters {
            s.field(2, "WiFi Adapter", wifi.name.bold());

            // SSID first — "which network am I on" is the most-scanned datum.
            if let Some(ref ssid) = wifi.ssid {
                s.field(4, "SSID", ssid.bright_cyan().bold());
            }

            s.field(4, "Standard", &wifi.wifi_standard);

            if !wifi.bands.is_empty() {
                let bands_str = wifi
                    .bands
                    .iter()
                    .map(|b| b.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                s.field(4, "Bands", bands_str);
            }

            // Current link speed
            if let Some(speed) = wifi.current_link_speed_mbps {
                let speed_str = if speed >= 1000 {
                    format!("{:.1} Gbps {}", speed as f32 / 1000.0, wifi.wifi_standard)
                } else {
                    format!("{} Mbps {}", speed, wifi.wifi_standard)
                };
                s.field(4, "Link Speed", speed_str.green());
            }

            // Chipset manufacturer
            if let Some(ref manufacturer) = wifi.chipset_manufacturer {
                s.field(4, "Chipset", manufacturer);
            }

            // IPv4 address
            if let Some(ref ipv4) = wifi.ipv4_address {
                s.field(4, "IPv4", ipv4.cyan());
            }

            // IPv6 address
            if let Some(ref ipv6) = wifi.ipv6_address {
                s.field(4, "IPv6", ipv6.cyan());
            }

            // Gateway latency (Wi-Fi tolerates more: warn ≥10, crit ≥50).
            if let Some(latency) = wifi.gateway_latency_ms {
                s.field(4, "Gateway Latency (LAN)", format_latency(latency, 10, 50));
            }
        }
    }

    if !net.ethernet_adapters.is_empty() {
        if !net.wifi_adapters.is_empty() {
            s.blank();
        }

        for eth in &net.ethernet_adapters {
            s.field(2, "Ethernet", eth.name.bold());

            if let Some(speed) = eth.speed_mbps {
                let speed_str = if speed >= 1000 {
                    format!("{:.1} Gbps", speed as f32 / 1000.0)
                } else {
                    format!("{} Mbps", speed)
                };
                s.field(4, "Speed", speed_str.green());
            }

            // IPv4 address
            if let Some(ref ipv4) = eth.ipv4_address {
                s.field(4, "IPv4", ipv4.cyan());
            }

            // IPv6 address
            if let Some(ref ipv6) = eth.ipv6_address {
                s.field(4, "IPv6", ipv6.cyan());
            }

            // Gateway latency (Ethernet should be tighter: warn ≥5, crit ≥20).
            if let Some(latency) = eth.gateway_latency_ms {
                s.field(4, "Gateway Latency (LAN)", format_latency(latency, 5, 20));
            }
        }
    }

    // Bluetooth adapters
    if !net.bluetooth_adapters.is_empty() {
        if !net.wifi_adapters.is_empty() || !net.ethernet_adapters.is_empty() {
            s.blank();
        }

        for bt in &net.bluetooth_adapters {
            let status_icon = if bt.is_enabled { "✓" } else { "✗" };
            let status_color = if bt.is_enabled {
                status_icon.green()
            } else {
                status_icon.red()
            };

            s.field(2, "Bluetooth", format!("{} {}", bt.name.bold(), status_color));
            s.field(4, "Version", &bt.version);

            if let Some(ref manufacturer) = bt.manufacturer {
                s.field(4, "Manufacturer", manufacturer);
            }
        }
    }

    if net.wifi_adapters.is_empty()
        && net.ethernet_adapters.is_empty()
        && net.bluetooth_adapters.is_empty()
    {
        s.raw("  No network adapters detected");
    }

    // Global WAN diagnostics (best-effort external lookups; absent offline).
    // Internet latency is the round trip to a public host (1.1.1.1) — what the
    // online speed tests call "ping" — as opposed to the per-adapter Gateway
    // Latency (LAN), which is just the first hop to your own router.
    if net.internet_latency_ms.is_some() || net.public_ip.is_some() {
        let has_adapters = !net.wifi_adapters.is_empty()
            || !net.ethernet_adapters.is_empty()
            || !net.bluetooth_adapters.is_empty();
        if has_adapters {
            s.blank();
        }

        if let Some(latency) = net.internet_latency_ms {
            s.field(2, "Internet Latency (WAN)", format_latency(latency, 60, 150));
        }
        if let Some(ref ip) = net.public_ip {
            s.field(2, "Public IP", ip.cyan());
        }
    }

    s.render();
}

fn print_storage_info(storage: &[StorageInfo]) {
    print_section_header("Storage");

    let mut s = Section::new();

    for (i, disk) in storage.iter().enumerate() {
        if i > 0 {
            s.blank();
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

        // Disk header keeps its own styling (cyan label + number), not a dim field.
        s.raw(format!("  {} {}: {}", "Disk".cyan().bold(), i, disk_title.bold()));

        // Technology type (HDD/SSD/NVMe)
        s.field(4, "Type", disk.disk_type.to_string().bold());

        // Interface (Bus type and speed)
        if let Some(ref bus_type) = disk.bus_type {
            if let Some(ref speed) = disk.interface_speed {
                s.field(4, "Interface", format!("{} - {}", bus_type, speed.to_string().green()));
            } else {
                s.field(4, "Interface", bus_type);
            }
        } else if let Some(ref speed) = disk.interface_speed {
            s.field(4, "Interface", speed.to_string().green());
        }

        // File System
        if !disk.file_system.is_empty() {
            s.field(4, "File System", &disk.file_system);
        }

        // Mount point and capacity
        s.field(4, "Mount Point", &disk.mount_point);
        s.field(4, "Capacity", format_bytes(disk.total_bytes));

        // Usage bar with percentage and free space
        s.field(
            4,
            "Usage",
            format!(
                "{} {:.1}% ({} free)",
                create_usage_bar(disk.usage_percent, 20),
                disk.usage_percent,
                format_bytes(disk.available_bytes)
            ),
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
            s.field(4, "Temperature", format!("{}°C", temp_color));
        }

        // Health status (if available)
        if let Some(ref status) = disk.smart_status {
            let status_str = match status {
                crate::core::system_info::types::SmartStatus::Healthy => "Healthy".green(),
                crate::core::system_info::types::SmartStatus::Warning => "Warning".yellow(),
                crate::core::system_info::types::SmartStatus::Critical => "Critical".red(),
                crate::core::system_info::types::SmartStatus::Unknown => "Unknown".normal(),
            };
            s.field(4, "Health", status_str);
        }

        // NVMe wear (PercentageUsed). Honest endurance estimate: 100 does NOT
        // mean "dead" and the value may exceed 100, so we only color the trend —
        // never imply failure.
        if let Some(wear) = disk.wear_percent {
            let wear_str = if wear >= 90 {
                format!("{}%", wear).red()
            } else if wear >= 70 {
                format!("{}%", wear).yellow()
            } else {
                format!("{}%", wear).green()
            };
            s.field(4, "Wear", wear_str);
        }

        // Power-on hours (if available)
        if let Some(hours) = disk.power_on_hours {
            s.field(
                4,
                "Power-On Hours",
                format!("{} hours ({:.1} days)", hours, hours as f64 / 24.0),
            );
        }

        // Data written/read (if available)
        if let Some(written) = disk.total_bytes_written {
            s.field(4, "Data Written", format_bytes(written));
        }
        if let Some(read) = disk.total_bytes_read {
            s.field(4, "Data Read", format_bytes(read));
        }

        // Serial number and firmware (if available)
        if let Some(ref serial) = disk.serial_number {
            s.field(4, "Serial", serial);
        }
        if let Some(ref firmware) = disk.firmware_version {
            s.field(4, "Firmware", firmware);
        }
    }

    // Show expansion capacity
    #[cfg(windows)]
    {
        if let Ok(slots) = crate::platform::system_info_windows::get_available_storage_slots() {
            s.blank();
            s.raw(format!("  {} ", "Expansion Capacity:".cyan().bold()));

            // SATA ports
            if let (Some(total), Some(available)) = (slots.sata_total, slots.sata_available) {
                let status = if available > 0 {
                    format!("{} available", available).green()
                } else {
                    "full".yellow()
                };
                s.field(
                    4,
                    "SATA Ports",
                    format!("{} used / {} total ({})", slots.sata_used, total, status),
                );

                if slots.sata_hot_swap {
                    s.field(6, "Hot-Swap", "Supported");
                }
            }

            // M.2 slots with detailed information
            if !slots.m2_slots.is_empty() {
                s.raw("    M.2 Slots:");
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

                    s.raw(format!(
                        "      Slot {}: {} {} {} - {}",
                        slot.slot_number, type_str, interface, form_factor, status
                    ));
                }
            }
        }
    }

    s.render();
}

fn print_os_info(os: &OsInfo) {
    print_section_header("Operating System");

    let mut s = Section::new();

    s.field(2, "Name", &os.name);

    if let Some(ref edition) = os.edition {
        s.field(2, "Edition", edition);
    }

    // Prefer the Windows feature-update version (e.g. "25H2"); fall back to the
    // generic sysinfo version string on other platforms.
    let version_line = os.display_version.as_deref().unwrap_or(os.version.as_str());
    s.field(2, "Version", version_line);

    if let Some(ref build) = os.build {
        s.field(2, "Build", build);
    }

    s.field(2, "Architecture", &os.architecture);

    if let Some(ref kernel) = os.kernel_version {
        s.field(2, "Kernel", kernel);
    }

    // HVCI (memory integrity). Running = green; configured-but-not-running =
    // yellow (a gap worth noticing); Off = neutral/dimmed (a fact, not a fault).
    if let Some(hvci) = os.hvci {
        let hvci_str = match hvci {
            HvciStatus::Running => hvci.to_string().green(),
            HvciStatus::ConfiguredNotRunning => hvci.to_string().yellow(),
            HvciStatus::Off => hvci.to_string().dimmed(),
        };
        s.field(2, "HVCI", hvci_str);
    }

    if let Some(ref user) = os.current_user {
        s.field(2, "User", user);
    }

    if let Some(uptime) = os.uptime_secs {
        s.field(2, "Uptime", format_uptime(uptime));
    }

    s.render();
}

/// Human-readable uptime, e.g. `5d 3h 12m`, `3h 12m`, `12m 4s`.
/// Drops trailing zero-units; falls back to seconds for very fresh boots.
fn format_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    let seconds = secs % 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

fn print_npu_info(npu: &NpuInfo) {
    print_section_header("NPU (Neural Processing Unit)");

    let mut s = Section::new();

    s.field(2, "Name", &npu.name);

    if let Some(tops) = npu.tops {
        s.field(
            2,
            "Performance",
            format!("{:.1} TOPS {}", tops, "(Architecture Spec)".dimmed()),
        );
    }

    s.render();
}

fn print_energy_info(battery: Option<&BatteryInfo>, power_plan: Option<&PowerPlanInfo>) {
    print_section_header("Energy");

    let mut s = Section::new();

    // Determine power source
    if let Some(battery) = battery {
        if battery.is_present {
            // Laptop with battery - show detailed battery information
            s.field(2, "Power Source", "Battery");

            s.blank();
            s.raw(format!("  {} ", "Battery Status".cyan().bold()));

            // Battery state with color and icon
            let (state_str, _state_icon) = match battery.state {
                BatteryState::Charging => (battery.state.to_string().green(), "⚡"),
                BatteryState::Discharging => (battery.state.to_string().yellow(), "🔋"),
                BatteryState::Full => (battery.state.to_string().green(), "✓"),
                BatteryState::NotCharging => (battery.state.to_string().cyan(), "○"),
                BatteryState::Unknown => (battery.state.to_string().normal(), "?"),
            };

            // Charge percentage appended to the status value
            let status_value = if let Some(percentage) = battery.percentage {
                let pct_str = if percentage >= 80 {
                    format!(" ({}%)", percentage).green()
                } else if percentage >= 20 {
                    format!(" ({}%)", percentage).yellow()
                } else {
                    format!(" ({}%)", percentage).red()
                };
                format!("{}{}", state_str, pct_str)
            } else {
                state_str.to_string()
            };
            s.field(4, "Status", status_value);

            // Time remaining or time to full
            if let Some(time_secs) = battery.time_remaining_secs {
                if time_secs > 0 && battery.state == BatteryState::Discharging {
                    let hours = time_secs / 3600;
                    let minutes = (time_secs % 3600) / 60;
                    s.field(4, "Time Remaining", format!("{}h {}m", hours, minutes));
                }
            }

            if let Some(time_secs) = battery.time_to_full_secs {
                if time_secs > 0 && battery.state == BatteryState::Charging {
                    let hours = time_secs / 3600;
                    let minutes = (time_secs % 3600) / 60;
                    s.field(4, "Time to Full Charge", format!("{}h {}m", hours, minutes));
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
                    s.field(4, "Power Draw", format!("{} (discharging)", power_str));
                } else if rate < 0 {
                    let watts = (-rate) as f32 / 1000.0;
                    s.field(4, "Charging Power", format!("{} W", format!("{:.1}", watts).green()));
                }
            }

            // Voltage information
            if let Some(voltage) = battery.voltage_mv {
                let volts = voltage as f32 / 1000.0;
                let voltage_value = if let Some(design_voltage) = battery.design_voltage_mv {
                    let design_volts = design_voltage as f32 / 1000.0;
                    format!("{:.2} V (nominal: {:.2} V)", volts, design_volts)
                } else {
                    format!("{:.2} V", volts)
                };
                s.field(4, "Voltage", voltage_value);
            }

            s.blank();
            s.raw(format!("  {} ", "Battery Specifications".cyan().bold()));

            // Capacity in Wh (easier to understand than mWh)
            if let (Some(design_mwh), Some(current_mwh)) =
                (battery.design_capacity_mwh, battery.full_charge_capacity_mwh)
            {
                let design_wh = design_mwh as f32 / 1000.0;
                let current_wh = current_mwh as f32 / 1000.0;

                s.field(4, "Design Capacity", format!("{:.1} Wh", design_wh));
                s.field(
                    4,
                    "Current Capacity",
                    format!(
                        "{:.1} Wh ({:.1}% of original)",
                        current_wh,
                        (current_wh / design_wh) * 100.0
                    ),
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

                s.field(4, "Battery Health", format!("{} (wear: {})", health_str, wear_str));
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
                s.field(4, "Cycle Count", format!("{} cycles ({})", cycles_str, lifecycle_str));
            }

            // Technology
            if let Some(ref tech) = battery.technology {
                s.field(4, "Technology", tech);
            }

            // Manufacturer info
            if let Some(ref mfg) = battery.manufacturer {
                s.field(4, "Manufacturer", mfg);
            }

            if let Some(ref serial) = battery.serial_number {
                s.field(4, "Serial Number", serial);
            }

            if let Some(ref date) = battery.manufacture_date {
                s.field(4, "Manufacture Date", date);
            }
        } else {
            // Desktop PC
            s.field(2, "Power Source", "AC Power (Mains)");
        }
    } else {
        // Desktop PC (no battery info available)
        s.field(2, "Power Source", "AC Power (Mains)");
    }

    // Power plan (for both laptops and desktops)
    if let Some(power) = power_plan {
        s.blank();
        let plan_str = match power.active_plan {
            PowerPlan::HighPerformance => power.active_plan.to_string().green(),
            PowerPlan::UltimatePerformance => power.active_plan.to_string().bright_green(),
            PowerPlan::PowerSaver => power.active_plan.to_string().cyan(),
            PowerPlan::Balanced => power.active_plan.to_string().yellow(),
            PowerPlan::Custom(_) => power.active_plan.to_string().normal(),
        };
        s.field(2, "Power Plan", plan_str);

        if let Some(ref mode) = power.power_mode {
            let mode_str = match mode {
                PowerMode::BestPerformance => mode.to_string().bright_green(),
                PowerMode::BetterPerformance => mode.to_string().green(),
                PowerMode::BetterBattery => mode.to_string().yellow(),
                PowerMode::BestPowerEfficiency => mode.to_string().cyan(),
            };
            s.field(2, "Power Mode", mode_str);
        }

        // CPU states
        if let (Some(min_state), Some(max_state)) =
            (power.processor_min_state, power.processor_max_state)
        {
            s.field(2, "CPU State", format!("{}% min, {}% max", min_state, max_state));
        } else if let Some(min_state) = power.processor_min_state {
            s.field(2, "CPU Min State", format!("{}%", min_state));
        } else if let Some(max_state) = power.processor_max_state {
            s.field(2, "CPU Max State", format!("{}%", max_state));
        }

        // Display and sleep timeouts (show differently for laptop vs desktop)
        let has_battery = battery.map(|b| b.is_present).unwrap_or(false);

        if has_battery {
            // Laptop: show both AC and DC timeouts
            if let (Some(ac), Some(dc)) =
                (power.display_timeout_ac_secs, power.display_timeout_dc_secs)
            {
                s.field(
                    2,
                    "Display Sleep",
                    format!("{} (AC), {} (Battery)", format_timeout(ac), format_timeout(dc)),
                );
            }

            if let (Some(ac), Some(dc)) = (power.sleep_timeout_ac_secs, power.sleep_timeout_dc_secs)
            {
                s.field(
                    2,
                    "System Sleep",
                    format!("{} (AC), {} (Battery)", format_timeout(ac), format_timeout(dc)),
                );
            }
        } else {
            // Desktop: show only AC timeouts
            if let Some(timeout) = power.display_timeout_ac_secs {
                s.field(2, "Display Sleep", format_timeout(timeout));
            }

            if let Some(timeout) = power.sleep_timeout_ac_secs {
                s.field(2, "System Sleep", format_timeout(timeout));
            }
        }
    }

    s.render();
}

/// Format a gateway latency with color thresholds. `0` means the OS reported a
/// sub-millisecond reply (`time<1ms`), so we show `<1 ms` rather than a bare
/// `0 ms`, which reads like "no data" instead of "blazing fast". `warn`/`crit`
/// are the yellow/red cut-offs (Wi-Fi tolerates more latency than Ethernet).
fn format_latency(ms: u32, warn: u32, crit: u32) -> ColoredString {
    let label = if ms == 0 {
        "<1 ms".to_string()
    } else {
        format!("{} ms", ms)
    };
    if ms >= crit {
        label.red()
    } else if ms >= warn {
        label.yellow()
    } else {
        label.green()
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

/// Map an AMD CPUID family id to its Zen microarchitecture — the GROUND TRUTH.
///
/// AMD defines the microarchitecture by CPUID family, so this is far more reliable
/// than pattern-matching the marketing name (`detect_cpu_microarchitecture`): a
/// "Ryzen AI 7 350" gives no numeric hint that it's Zen 5, but its family `0x1A`
/// does, by definition.
///
/// Family alone cannot always pin the exact generation, and we DON'T pretend it
/// can — `0x19` is shared by Zen 3 AND Zen 4 (reported honestly as `Zen 3/4`), and
/// `0x17` covers Zen / Zen+ / Zen 2 (reported as `Zen`). Disambiguating those would
/// need per-model ranges — the same maintenance burden as the string heuristic — so
/// we stop at what the family proves. `None` for any family we don't map, letting
/// the caller fall back to the marketing-string heuristic.
fn amd_microarch_from_family(family: u8) -> Option<String> {
    match family {
        0x1A => Some("Zen 5".to_string()),
        0x19 => Some("Zen 3/4".to_string()),
        0x17 => Some("Zen".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::amd_microarch_from_family;

    #[test]
    fn maps_zen5_from_family_1a() {
        // AMD Zen 5 (Ryzen 9000 / Ryzen AI 300) reports CPUID family 0x1A.
        assert_eq!(amd_microarch_from_family(0x1A), Some("Zen 5".to_string()));
    }

    #[test]
    fn maps_shared_family_19_honestly() {
        // 0x19 is BOTH Zen 3 and Zen 4 — family alone can't separate them.
        assert_eq!(amd_microarch_from_family(0x19), Some("Zen 3/4".to_string()));
    }

    #[test]
    fn maps_family_17_to_generic_zen() {
        // 0x17 spans Zen / Zen+ / Zen 2.
        assert_eq!(amd_microarch_from_family(0x17), Some("Zen".to_string()));
    }

    #[test]
    fn unknown_family_falls_through_to_none() {
        // Older (0x15 Bulldozer) or future families aren't mapped — caller falls
        // back to the marketing-string heuristic instead of guessing.
        assert_eq!(amd_microarch_from_family(0x15), None);
        assert_eq!(amd_microarch_from_family(0x00), None);
    }
}
