use crate::core::system_info::collector;
use crate::ui::system_formatters::{self, DisplayFilter};
use anyhow::Result;
use clap::ArgMatches;

pub mod monitor;

pub fn execute(matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("monitor", sub_matches)) => monitor::execute(sub_matches),
        Some(("info", sub_matches)) => execute_info(sub_matches),
        _ => {
            println!("Use 'msc sys --help' for more information.");
            Ok(())
        }
    }
}

fn execute_info(matches: &ArgMatches) -> Result<()> {
    println!("Collecting system information...\n");

    let system_info = collector::collect_system_info()?;

    // Parse filter flags
    let show_cpu = matches.get_flag("cpu");
    let show_gpu = matches.get_flag("gpu");
    let show_ram = matches.get_flag("ram");
    let show_mbo = matches.get_flag("mbo");
    let show_network = matches.get_flag("network");
    let show_os = matches.get_flag("os");
    let show_energy = matches.get_flag("energy");

    // If no flags are set, show everything
    let filter = if !show_cpu
        && !show_gpu
        && !show_ram
        && !show_mbo
        && !show_network
        && !show_os
        && !show_energy
    {
        DisplayFilter::all()
    } else {
        DisplayFilter {
            cpu: show_cpu,
            gpu: show_gpu,
            memory: show_ram,
            motherboard: show_mbo,
            network: show_network,
            storage: false,
            os: show_os,
            npu: false,
            energy: show_energy,
        }
    };

    system_formatters::format_system_info(&system_info, &filter);

    Ok(())
}
