use crate::core::system_info::types::{
    BatteryInfo, BatteryState, BatteryTechnology, PowerMode, PowerPlan, PowerPlanInfo,
};
use crate::error::{MscError, Result};
use serde::Deserialize;
use wmi::WMIConnection;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32Battery {
    battery_status: Option<u16>,
    estimated_charge_remaining: Option<u32>,
    estimated_run_time: Option<u32>,
    chemistry: Option<u16>,
    design_voltage: Option<u32>,
    manufacturer: Option<String>,
    serial_number: Option<String>,
    #[serde(rename = "ManufactureDate")]
    manufacture_date: Option<String>,
}

fn empty_battery() -> BatteryInfo {
    BatteryInfo {
        is_present: false,
        state: BatteryState::Unknown,
        percentage: None,
        time_remaining_secs: None,
        time_to_full_secs: None,
        design_capacity_mwh: None,
        full_charge_capacity_mwh: None,
        health_percentage: None,
        cycle_count: None,
        technology: None,
        voltage_mv: None,
        design_voltage_mv: None,
        discharge_rate_mw: None,
        manufacturer: None,
        serial_number: None,
        manufacture_date: None,
    }
}

/// Get comprehensive battery information using multiple Windows APIs.
///
/// Combines:
/// 1. Win32_Battery via direct WMI (basic status and state)
/// 2. BatteryStaticData / BatteryFullChargedCapacity via WMI (accurate capacity)
/// 3. Registry (cycle count and additional metrics)
pub fn get_battery_info() -> Result<BatteryInfo> {
    let wmi = WMIConnection::new()
        .map_err(|e| MscError::other(format!("WMI connect failed: {}", e)))?;

    let batteries: Vec<Win32Battery> = wmi
        .query()
        .map_err(|e| MscError::other(format!("WMI query failed: {}", e)))?;

    let battery = match batteries.into_iter().next() {
        Some(b) => b,
        None => return Ok(empty_battery()), // No battery → desktop PC
    };

    let state = match battery.battery_status {
        Some(1) => BatteryState::Discharging,
        Some(2) => BatteryState::Charging,
        Some(3) => BatteryState::Full,
        Some(11) => BatteryState::NotCharging,
        _ => BatteryState::Unknown,
    };

    let percentage = battery.estimated_charge_remaining.map(|p| p as u8);

    // EstimatedRunTime returns a sentinel ~71582788 when not discharging — filter it out.
    let time_remaining_secs = battery.estimated_run_time.and_then(|t| {
        if t < 71_582_788 {
            Some(t * 60)
        } else {
            None
        }
    });

    let technology = match battery.chemistry {
        Some(1) => Some(BatteryTechnology::LeadAcid),
        Some(2) => Some(BatteryTechnology::NickelCadmium),
        Some(3) => Some(BatteryTechnology::NickelMetalHydride),
        Some(4) | Some(6) => Some(BatteryTechnology::LithiumIon),
        Some(7) => Some(BatteryTechnology::LithiumPolymer),
        _ => Some(BatteryTechnology::Unknown),
    };

    let (design_capacity, full_charge_capacity, cycle_count) = get_battery_capacity_wmi();

    let health = match (design_capacity, full_charge_capacity) {
        (Some(design), Some(full)) if design > 0 => {
            Some(((full as f32 / design as f32) * 100.0).min(100.0) as u8)
        }
        _ => None,
    };

    let (voltage_mv, discharge_rate_mw) = get_battery_power_info();

    let design_voltage = battery.design_voltage.or(voltage_mv);

    let nz = |s: Option<String>| s.filter(|v| !v.is_empty());

    Ok(BatteryInfo {
        is_present: true,
        state,
        percentage,
        time_remaining_secs,
        time_to_full_secs: None,
        design_capacity_mwh: design_capacity,
        full_charge_capacity_mwh: full_charge_capacity,
        health_percentage: health,
        cycle_count,
        technology,
        voltage_mv,
        design_voltage_mv: design_voltage,
        discharge_rate_mw,
        manufacturer: nz(battery.manufacturer),
        serial_number: nz(battery.serial_number),
        manufacture_date: nz(battery.manufacture_date),
    })
}

/// Get battery capacity using BatteryStaticData and BatteryFullChargedCapacity WMI classes
///
/// These classes provide more accurate capacity information than Win32_Battery
fn get_battery_capacity_wmi() -> (Option<u32>, Option<u32>, Option<u32>) {
    use std::process::Command;

    // Try to get design capacity and full charge capacity
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$battery = Get-WmiObject -Namespace root\\wmi -Class BatteryStaticData | Select-Object -First 1; \
             $full = Get-WmiObject -Namespace root\\wmi -Class BatteryFullChargedCapacity | Select-Object -First 1; \
             $cycle = Get-WmiObject -Namespace root\\wmi -Class BatteryCycleCount -ErrorAction SilentlyContinue | Select-Object -First 1; \
             @{ \
                 DesignedCapacity = $battery.DesignedCapacity; \
                 FullChargedCapacity = $full.FullChargedCapacity; \
                 CycleCount = $cycle.CycleCount \
             } | ConvertTo-Json"
        ])
        .output()
        .ok();

    if let Some(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
            let design = json["DesignedCapacity"].as_u64().map(|c| c as u32);
            let full = json["FullChargedCapacity"].as_u64().map(|c| c as u32);
            let cycles = json["CycleCount"].as_u64().map(|c| c as u32);

            return (design, full, cycles);
        }
    }

    // Fallback: try older method
    (None, None, None)
}

/// Get real-time battery power information (voltage and discharge rate)
fn get_battery_power_info() -> (Option<u32>, Option<i32>) {
    use std::process::Command;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$status = Get-WmiObject -Namespace root\\wmi -Class BatteryStatus | Select-Object -First 1; \
             @{ \
                 Voltage = $status.Voltage; \
                 DischargeRate = $status.DischargeRate; \
                 ChargeRate = $status.ChargeRate \
             } | ConvertTo-Json"
        ])
        .output()
        .ok();

    if let Some(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
            let voltage = json["Voltage"].as_u64().map(|v| v as u32);

            // DischargeRate is positive when discharging, ChargeRate when charging
            let discharge = json["DischargeRate"].as_u64().map(|r| r as i32);
            let charge = json["ChargeRate"].as_u64().map(|r| -(r as i32));

            let rate = discharge.or(charge);

            return (voltage, rate);
        }
    }

    (None, None)
}

/// Get power plan information
pub fn get_power_plan_info() -> Result<PowerPlanInfo> {
    use std::process::Command;

    // Use powercfg to get the active power plan
    let output = Command::new("powercfg")
        .args(["/getactivescheme"])
        .output()
        .map_err(|e| MscError::other(format!("Failed to run powercfg: {}", e)))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Parse output: "Power Scheme GUID: <guid>  (Plan Name)"
    let active_plan_name = output_str
        .lines()
        .next()
        .and_then(|line| line.split('(').nth(1))
        .and_then(|s| s.split(')').next())
        .unwrap_or("Balanced")
        .trim()
        .to_string();

    let active_plan = match active_plan_name.to_lowercase().as_str() {
        s if s.contains("balanced") || s.contains("equilibrado") => PowerPlan::Balanced,
        s if s.contains("high") && s.contains("performance") => PowerPlan::HighPerformance,
        s if s.contains("best") && s.contains("performance") => PowerPlan::HighPerformance,
        s if s.contains("alto") && s.contains("rendimiento") => PowerPlan::HighPerformance,
        s if s.contains("power saver") || s.contains("ahorro") => PowerPlan::PowerSaver,
        s if s.contains("ultimate") => PowerPlan::UltimatePerformance,
        _ => PowerPlan::Custom(active_plan_name.clone()),
    };

    // Try to get Windows 11 Power Mode (Best Performance, Better Performance, Better Battery, etc.)
    let power_mode_output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Power\\User\\PowerSchemes' -Name ActiveOverlayAcPowerScheme -ErrorAction SilentlyContinue | Select-Object -ExpandProperty ActiveOverlayAcPowerScheme"
        ])
        .output()
        .ok();

    let power_mode = if let Some(output) = power_mode_output {
        let mode_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

        match mode_str.to_lowercase().as_str() {
            s if s.contains("54533251-82be-4824-96c1-47b60b740d00") => {
                Some(PowerMode::BestPerformance)
            }
            s if s.contains("ded574b5-45a0-4f42-8737-46345c09c238") => {
                Some(PowerMode::BestPerformance)
            } // Best Performance (alternative GUID)
            s if s.contains("3af9b8d9-7c97-431d-ad78-34a8bfea439f") => {
                Some(PowerMode::BetterPerformance)
            }
            s if s.contains("961cc777-2547-4f9d-8174-7d86181b8a7a") => {
                Some(PowerMode::BetterBattery)
            }
            s if s.contains("00000000-0000-0000-0000-000000000000") => {
                Some(PowerMode::BestPowerEfficiency)
            }
            _ => None,
        }
    } else {
        None
    };

    Ok(PowerPlanInfo {
        active_plan,
        power_mode,
        processor_min_state: None,
        processor_max_state: None,
        display_brightness_ac: None,
        display_brightness_dc: None,
        display_timeout_ac_secs: None,
        display_timeout_dc_secs: None,
        sleep_timeout_ac_secs: None,
        sleep_timeout_dc_secs: None,
    })
}
