//! System monitor command handler.
//!
//! Provides real-time system monitoring in a TUI dashboard.

use anyhow::{Context, Result};
use clap::ArgMatches;

use crate::ui::monitor_tui::{run_monitor_app, MonitorAppConfig};

/// Execute the monitor command
pub fn execute(matches: &ArgMatches) -> Result<()> {
    // Extract arguments
    let interval = matches.get_one::<u64>("interval").copied().unwrap_or(1000);

    let cpu_only = matches.get_flag("cpu-only");
    let gpu_only = matches.get_flag("gpu-only");
    let memory_only = matches.get_flag("memory-only");

    let show_network = matches.get_flag("network");
    let show_disks = matches.get_flag("disks");

    let top_processes = matches
        .get_one::<usize>("top-processes")
        .copied()
        .unwrap_or(10);

    let json_output = matches.get_flag("json");

    // Handle JSON output mode (non-TUI)
    if json_output {
        return run_json_output(interval);
    }

    // Determine what to show based on flags
    let (show_cpu, show_memory, show_gpu) = if cpu_only || gpu_only || memory_only {
        (cpu_only, memory_only, gpu_only)
    } else {
        (true, true, true)
    };

    // Build config
    let config = MonitorAppConfig {
        interval_ms: interval,
        show_cpu,
        show_memory,
        show_gpu,
        show_disks: show_disks || !(cpu_only || gpu_only || memory_only),
        show_network: show_network || !(cpu_only || gpu_only || memory_only),
        show_temperatures: true,
        show_processes: !(cpu_only || gpu_only || memory_only),
        top_processes,
    };

    // Run TUI
    run_monitor_app(config).context("Failed to run system monitor")
}

/// Run in JSON output mode (for scripting)
fn run_json_output(interval_ms: u64) -> Result<()> {
    use crate::core::system_monitor::{CollectorConfig, MetricsCollector};
    use std::time::Duration;

    let mut collector = MetricsCollector::with_config(CollectorConfig::default());

    // Initial collection
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);

    loop {
        let metrics = collector.collect()?;
        println!("{}", serde_json::to_string(&metrics)?);

        std::thread::sleep(Duration::from_millis(interval_ms));
    }
}
