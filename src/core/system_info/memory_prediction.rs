use crate::core::system_info::types::*;

/// Predict maximum memory capacity based on multiple heuristics
pub fn predict_memory_capacity(
    cpu_model: &str,
    motherboard_model: Option<&str>,
    ddr_type: Option<DdrType>,
    modules: &[MemoryModule],
    total_slots: Option<u32>,
    smbios_max_capacity: Option<u64>,
) -> MemoryPrediction {
    let mut justification = Vec::new();
    let mut warnings = Vec::new();

    // 1. Detect CPU limits
    let cpu_limits = detect_cpu_memory_limits(cpu_model);
    let cpu_limit_bytes = cpu_limits
        .as_ref()
        .map(|l| (l.max_capacity_gb as u64) * 1024 * 1024 * 1024);

    if let Some(ref limits) = cpu_limits {
        justification.push(format!(
            "CPU supports up to {} GB (detected from model)",
            limits.max_capacity_gb
        ));
    } else {
        warnings.push("CPU model not recognized - using conservative estimates".to_string());
    }

    // 2. Detect Chipset limits
    let chipset_limit_gb = detect_chipset_limits(motherboard_model, cpu_model);
    let chipset_limit_bytes = chipset_limit_gb.map(|gb| gb * 1024 * 1024 * 1024);

    if let Some(chipset_gb) = chipset_limit_gb {
        justification.push(format!("Chipset supports up to {} GB", chipset_gb));
    } else if motherboard_model.is_some() {
        warnings.push("Chipset limits could not be determined from motherboard model".to_string());
    }

    // 3. Get DDR generation limits
    let ddr_limits = if let Some(ddr) = ddr_type {
        get_ddr_generation_limits(ddr)
    } else {
        warnings.push("DDR type unknown - assuming DDR4 limits".to_string());
        get_ddr_generation_limits(DdrType::DDR4)
    };

    let ddr_generation_limit_bytes = (ddr_limits.max_module_capacity_gb as u64)
        * (total_slots.unwrap_or(2) as u64)
        * 1024
        * 1024
        * 1024;

    justification.push(format!(
        "{} supports up to {} GB per module",
        ddr_limits.ddr_type, ddr_limits.max_module_capacity_gb
    ));

    // 4. Calculate minimum guaranteed capacity
    let minimum_guaranteed_bytes = calculate_minimum_guaranteed(modules, total_slots.unwrap_or(2));

    if !modules.is_empty() {
        let max_module_gb = modules
            .iter()
            .map(|m| m.capacity_bytes / (1024 * 1024 * 1024))
            .max()
            .unwrap_or(0);

        justification.push(format!(
            "Minimum guaranteed: {} GB (based on {} x {} GB modules installed)",
            minimum_guaranteed_bytes / (1024 * 1024 * 1024),
            total_slots.unwrap_or(2),
            max_module_gb
        ));
    }

    // 5. Validate SMBIOS capacity
    let is_smbios_validated = if let Some(smbios_capacity) = smbios_max_capacity {
        validate_smbios_capacity(
            smbios_capacity,
            minimum_guaranteed_bytes,
            ddr_generation_limit_bytes,
            cpu_limit_bytes,
            &mut justification,
            &mut warnings,
        )
    } else {
        warnings.push("SMBIOS MaxCapacity not available".to_string());
        false
    };

    // 6. Calculate final prediction
    let mut candidates = Vec::new();

    // Add CPU limit
    if let Some(cpu_limit) = cpu_limit_bytes {
        candidates.push(cpu_limit);
    }

    // Add chipset limit
    if let Some(chipset_limit) = chipset_limit_bytes {
        candidates.push(chipset_limit);
    }

    // Add DDR generation limit
    candidates.push(ddr_generation_limit_bytes);

    // Add SMBIOS if validated
    if is_smbios_validated {
        if let Some(smbios) = smbios_max_capacity {
            candidates.push(smbios);
        }
    }

    // Use the minimum of all limits (most restrictive)
    let maximum_predicted_bytes = candidates
        .into_iter()
        .min()
        .unwrap_or(ddr_generation_limit_bytes);

    // Ensure predicted max is at least the minimum guaranteed
    let maximum_predicted_bytes = maximum_predicted_bytes.max(minimum_guaranteed_bytes);

    // 7. Determine prediction method and confidence
    let (prediction_method, confidence) = calculate_confidence_and_method(
        cpu_limits.is_some(),
        ddr_type.is_some(),
        is_smbios_validated,
        !modules.is_empty(),
    );

    // 8. Add final recommendation
    let recommended_config = calculate_recommended_config(
        maximum_predicted_bytes,
        total_slots.unwrap_or(2),
        &ddr_limits,
    );

    justification.push(format!(
        "Recommended configuration: {} (total {} GB)",
        recommended_config.0, recommended_config.1
    ));

    MemoryPrediction {
        minimum_guaranteed_bytes,
        maximum_predicted_bytes,
        smbios_reported_bytes: smbios_max_capacity,
        cpu_limit_bytes,
        ddr_generation_limit_bytes,
        chipset_limit_bytes,
        confidence,
        justification,
        warnings,
        is_smbios_validated,
        prediction_method,
    }
}

/// Detect CPU memory controller limits based on CPU model
pub fn detect_cpu_memory_limits(cpu_model: &str) -> Option<CpuMemoryLimits> {
    let model_lower = cpu_model.to_lowercase();

    // Intel CPUs
    if model_lower.contains("intel") {
        // Core Ultra (14th gen and beyond) - Arrow Lake, Meteor Lake
        if model_lower.contains("ultra") {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 192, // Arrow Lake supports up to 192GB
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR5, DdrType::DDR4],
            });
        }

        // 14th Gen (Raptor Lake Refresh)
        if model_lower.contains("14900")
            || model_lower.contains("14700")
            || model_lower.contains("14600")
            || model_lower.contains("14400")
        {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 192,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR5, DdrType::DDR4],
            });
        }

        // 13th Gen (Raptor Lake)
        if model_lower.contains("13900")
            || model_lower.contains("13700")
            || model_lower.contains("13600")
            || model_lower.contains("13400")
        {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 128,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR5, DdrType::DDR4],
            });
        }

        // 12th Gen (Alder Lake)
        if model_lower.contains("12900")
            || model_lower.contains("12700")
            || model_lower.contains("12600")
            || model_lower.contains("12400")
            || model_lower.contains("12100")
        {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 128,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR5, DdrType::DDR4],
            });
        }

        // 11th Gen (Rocket Lake)
        if model_lower.contains("11900")
            || model_lower.contains("11700")
            || model_lower.contains("11600")
            || model_lower.contains("11400")
        {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 128,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR4],
            });
        }

        // 10th Gen (Comet Lake)
        if model_lower.contains("10900")
            || model_lower.contains("10700")
            || model_lower.contains("10600")
            || model_lower.contains("10400")
        {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 128,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR4],
            });
        }

        // 9th Gen and earlier (Coffee Lake, etc.)
        if model_lower.contains("9900")
            || model_lower.contains("9700")
            || model_lower.contains("8700")
            || model_lower.contains("7700")
        {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 64,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR4],
            });
        }
    }

    // AMD Ryzen CPUs
    if model_lower.contains("ryzen") || model_lower.contains("amd") {
        // Ryzen AI 300 series (Zen 5) - Strix Point
        if model_lower.contains("ai 9")
            || model_lower.contains("ai 7")
            || model_lower.contains("ai 3")
        {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 256,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR5],
            });
        }

        // Ryzen 9000 series (Zen 5)
        if model_lower.contains("9950")
            || model_lower.contains("9900")
            || model_lower.contains("9700")
            || model_lower.contains("9600")
        {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 256,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR5],
            });
        }

        // Ryzen 7000 series (Zen 4)
        if model_lower.contains("7950")
            || model_lower.contains("7900")
            || model_lower.contains("7700")
            || model_lower.contains("7600")
        {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 128,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR5],
            });
        }

        // Ryzen 5000 series (Zen 3)
        if model_lower.contains("5950")
            || model_lower.contains("5900")
            || model_lower.contains("5800")
            || model_lower.contains("5700")
            || model_lower.contains("5600")
        {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 128,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR4],
            });
        }

        // Ryzen 3000 series (Zen 2)
        if model_lower.contains("3950")
            || model_lower.contains("3900")
            || model_lower.contains("3800")
            || model_lower.contains("3700")
            || model_lower.contains("3600")
        {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 128,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR4],
            });
        }

        // Ryzen 2000 series (Zen+)
        if model_lower.contains("2700") || model_lower.contains("2600") {
            return Some(CpuMemoryLimits {
                max_capacity_gb: 64,
                max_channels: 2,
                supported_ddr_types: vec![DdrType::DDR4],
            });
        }
    }

    None
}

/// Get DDR generation limits based on JEDEC standards and market availability
pub fn get_ddr_generation_limits(ddr_type: DdrType) -> DdrGenerationLimits {
    match ddr_type {
        DdrType::DDR5 => DdrGenerationLimits {
            ddr_type: DdrType::DDR5,
            max_module_capacity_gb: 64, // Current max as of 2025
            common_capacities_gb: vec![8, 16, 24, 32, 48, 64],
        },
        DdrType::DDR4 => DdrGenerationLimits {
            ddr_type: DdrType::DDR4,
            max_module_capacity_gb: 32, // Standard consumer max (64GB for server)
            common_capacities_gb: vec![4, 8, 16, 32],
        },
        DdrType::DDR3 => DdrGenerationLimits {
            ddr_type: DdrType::DDR3,
            max_module_capacity_gb: 16,
            common_capacities_gb: vec![2, 4, 8, 16],
        },
        DdrType::DDR2 => DdrGenerationLimits {
            ddr_type: DdrType::DDR2,
            max_module_capacity_gb: 4,
            common_capacities_gb: vec![1, 2, 4],
        },
        DdrType::DDR => DdrGenerationLimits {
            ddr_type: DdrType::DDR,
            max_module_capacity_gb: 1,
            common_capacities_gb: vec![256, 512, 1024], // MB values
        },
        DdrType::Unknown => DdrGenerationLimits {
            ddr_type: DdrType::Unknown,
            max_module_capacity_gb: 16, // Conservative fallback (DDR3 level)
            common_capacities_gb: vec![4, 8, 16],
        },
    }
}

/// Calculate minimum guaranteed capacity based on installed modules
pub fn calculate_minimum_guaranteed(modules: &[MemoryModule], total_slots: u32) -> u64 {
    if modules.is_empty() {
        return 0;
    }

    // Find the largest installed module
    let max_module_capacity = modules.iter().map(|m| m.capacity_bytes).max().unwrap_or(0);

    // Minimum guaranteed = largest module × total slots
    max_module_capacity * (total_slots as u64)
}

/// Validate SMBIOS MaxCapacity value
pub fn validate_smbios_capacity(
    smbios_capacity: u64,
    minimum_guaranteed: u64,
    ddr_generation_limit: u64,
    cpu_limit: Option<u64>,
    justification: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> bool {
    // Rule 1: Must not be zero
    if smbios_capacity == 0 {
        warnings.push("SMBIOS MaxCapacity is zero - ignoring".to_string());
        return false;
    }

    // Rule 2: Must not contradict minimum guaranteed
    if smbios_capacity < minimum_guaranteed {
        warnings.push(format!(
            "SMBIOS MaxCapacity ({} GB) is less than minimum guaranteed ({} GB) - ignoring",
            smbios_capacity / (1024 * 1024 * 1024),
            minimum_guaranteed / (1024 * 1024 * 1024)
        ));
        return false;
    }

    // Rule 3: Must not exceed more than 2x the DDR generation limit
    if smbios_capacity > ddr_generation_limit * 2 {
        warnings.push(format!(
            "SMBIOS MaxCapacity ({} GB) exceeds realistic DDR limits - ignoring",
            smbios_capacity / (1024 * 1024 * 1024)
        ));
        return false;
    }

    // Rule 4: Must not contradict CPU limit
    if let Some(cpu_lim) = cpu_limit {
        if smbios_capacity > cpu_lim * 2 {
            warnings.push(format!(
                "SMBIOS MaxCapacity ({} GB) exceeds CPU limits - ignoring",
                smbios_capacity / (1024 * 1024 * 1024)
            ));
            return false;
        }
    }

    // Rule 5: Should be a reasonable power of 2 or multiple of common sizes
    let capacity_gb = smbios_capacity / (1024 * 1024 * 1024);
    let is_reasonable = capacity_gb.is_power_of_two()
        || capacity_gb.is_multiple_of(16)
        || capacity_gb.is_multiple_of(24);

    if !is_reasonable {
        warnings.push(format!(
            "SMBIOS MaxCapacity ({} GB) is an unusual value - treating with caution",
            capacity_gb
        ));
    }

    justification.push(format!(
        "SMBIOS reports {} GB maximum capacity (validated)",
        capacity_gb
    ));

    true
}

/// Calculate confidence level and prediction method
pub fn calculate_confidence_and_method(
    cpu_known: bool,
    ddr_known: bool,
    smbios_validated: bool,
    has_modules: bool,
) -> (PredictionMethod, ConfidenceLevel) {
    match (cpu_known, ddr_known, smbios_validated, has_modules) {
        (true, true, true, _) => (PredictionMethod::CpuAndDdrLimits, ConfidenceLevel::High),
        (true, true, false, _) => (PredictionMethod::CpuAndDdrLimits, ConfidenceLevel::Medium),
        (false, true, true, _) => (PredictionMethod::SmbiosValidated, ConfidenceLevel::Medium),
        (_, true, false, true) => (PredictionMethod::DdrAndSlots, ConfidenceLevel::Medium),
        _ => (PredictionMethod::Conservative, ConfidenceLevel::Low),
    }
}

/// Calculate recommended configuration
fn calculate_recommended_config(
    max_capacity: u64,
    total_slots: u32,
    ddr_limits: &DdrGenerationLimits,
) -> (String, u64) {
    let max_gb = max_capacity / (1024 * 1024 * 1024);
    let gb_per_slot = max_gb / (total_slots as u64);

    // Find the closest common capacity
    let recommended_per_slot = ddr_limits
        .common_capacities_gb
        .iter()
        .filter(|&&cap| (cap as u64) <= gb_per_slot)
        .max()
        .copied()
        .unwrap_or(ddr_limits.max_module_capacity_gb);

    let total_recommended = (recommended_per_slot as u64) * (total_slots as u64);

    (
        format!("{} x {} GB modules", total_slots, recommended_per_slot),
        total_recommended,
    )
}

/// Detect chipset memory limits
pub fn detect_chipset_limits(motherboard_model: Option<&str>, cpu_model: &str) -> Option<u64> {
    let cpu_lower = cpu_model.to_lowercase();

    // Intel Chipsets
    if cpu_lower.contains("intel") {
        // Z890/W880 (Arrow Lake - LGA 1851)
        if cpu_lower.contains("ultra 2")
            || cpu_lower.contains("ultra 9 2")
            || cpu_lower.contains("ultra 7 2")
            || cpu_lower.contains("ultra 5 2")
        {
            return Some(192); // 192GB max
        }

        // Z790/Z690/B760/B660 (LGA 1700 - 12th/13th/14th gen)
        if cpu_lower.contains("14900")
            || cpu_lower.contains("14700")
            || cpu_lower.contains("13900")
            || cpu_lower.contains("13700")
            || cpu_lower.contains("12900")
            || cpu_lower.contains("12700")
        {
            // Check motherboard model if available
            if let Some(mb) = motherboard_model {
                let mb_lower = mb.to_lowercase();
                if mb_lower.contains("z790") || mb_lower.contains("z690") {
                    return Some(192); // 192GB on Z-series
                } else if mb_lower.contains("b760") || mb_lower.contains("b660") {
                    return Some(128); // 128GB on B-series
                } else if mb_lower.contains("h770")
                    || mb_lower.contains("h670")
                    || mb_lower.contains("h610")
                {
                    return Some(128); // 128GB on H-series
                }
            }
            // Default for LGA 1700
            return Some(128);
        }

        // Z590/Z490/B560/B460 (LGA 1200 - 10th/11th gen)
        if cpu_lower.contains("11900")
            || cpu_lower.contains("11700")
            || cpu_lower.contains("10900")
            || cpu_lower.contains("10700")
        {
            // All LGA 1200 chipsets support 128GB
            return Some(128);
        }

        // Z390/Z370/B365/H370 (LGA 1151 - 8th/9th gen)
        if cpu_lower.contains("9900") || cpu_lower.contains("9700") || cpu_lower.contains("8700") {
            return Some(128); // 128GB max
        }

        // Older chipsets (Z270, Z170, etc.)
        if cpu_lower.contains("7700") || cpu_lower.contains("6700") {
            return Some(64); // 64GB max
        }
    }

    // AMD Chipsets
    if cpu_lower.contains("ryzen") || cpu_lower.contains("amd") {
        // X870/X870E/B850 (Ryzen 9000/8000 series - AM5)
        if cpu_lower.contains("9950")
            || cpu_lower.contains("9900")
            || cpu_lower.contains("9700")
            || cpu_lower.contains("9600")
        {
            if let Some(mb) = motherboard_model {
                let mb_lower = mb.to_lowercase();
                if mb_lower.contains("x870") || mb_lower.contains("x670") {
                    return Some(256); // 256GB on X-series
                } else if mb_lower.contains("b850") || mb_lower.contains("b650") {
                    return Some(192); // 192GB on B-series
                }
            }
            return Some(192); // Default for AM5
        }

        // X670/B650/A620 (Ryzen 7000 series - AM5)
        if cpu_lower.contains("7950")
            || cpu_lower.contains("7900")
            || cpu_lower.contains("7700")
            || cpu_lower.contains("7600")
        {
            if let Some(mb) = motherboard_model {
                let mb_lower = mb.to_lowercase();
                if mb_lower.contains("x670") {
                    return Some(192); // 192GB on X670
                } else if mb_lower.contains("b650") {
                    return Some(128); // 128GB on B650
                } else if mb_lower.contains("a620") {
                    return Some(96); // 96GB on A620
                }
            }
            return Some(128); // Default for AM5
        }

        // X570/B550/A520/X470/B450 (Ryzen 5000/3000 series - AM4)
        if cpu_lower.contains("5950")
            || cpu_lower.contains("5900")
            || cpu_lower.contains("5800")
            || cpu_lower.contains("5700")
            || cpu_lower.contains("5600")
            || cpu_lower.contains("3950")
            || cpu_lower.contains("3900")
            || cpu_lower.contains("3700")
        {
            if let Some(mb) = motherboard_model {
                let mb_lower = mb.to_lowercase();
                // A520 has lower memory limit
                if mb_lower.contains("a520") {
                    return Some(64); // 64GB on A520
                }
                // X570/X470/B550/B450 all support 128GB
            }
            return Some(128); // Default for AM4 (128GB)
        }

        // Older AM4 chipsets
        if cpu_lower.contains("2700") || cpu_lower.contains("2600") {
            return Some(64); // 64GB max for older Ryzen
        }
    }

    None
}
