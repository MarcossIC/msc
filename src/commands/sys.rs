use crate::core::system_info::{
    cache, collect_system_info_with_profile_progress, format_profile_report, TOTAL_SECTIONS,
};
use crate::ui::spinner::Spinner;
use crate::ui::system_formatters::{self, DisplayFilter};
use anyhow::Result;
use clap::ArgMatches;
use colored::Colorize;

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
    let profile = matches.get_flag("profile");
    let no_cache = matches.get_flag("no-cache");
    let clear_cache = matches.get_flag("clear-cache");
    let json = matches.get_flag("json");
    let compact = matches.get_flag("compact");
    // WAN probes (public IP + internet latency) are opt-in: they add an external
    // network round-trip (~370ms) the normal path shouldn't pay for.
    let wan = matches.get_flag("wan");

    if clear_cache {
        cache::clear_all();
    }
    if no_cache {
        cache::set_enabled(false);
    }

    // JSON mode: stdout must stay parseable, so no spinner and no banner.
    let (system_info, timings) = if json {
        collect_system_info_with_profile_progress(None, wan)?
    } else {
        let spinner = Spinner::start("Collecting system information", TOTAL_SECTIONS);
        let progress = spinner.progress_handle();
        let result = collect_system_info_with_profile_progress(Some(progress), wan)?;
        spinner.finish();
        result
    };

    if json {
        let payload = if compact {
            serde_json::to_string(&system_info)?
        } else {
            serde_json::to_string_pretty(&system_info)?
        };
        println!("{}", payload);
        if profile {
            // Profile report goes to stderr so JSON consumers stay clean.
            eprintln!("{}", format_profile_report(&timings));
        }
        return Ok(());
    }

    let filter = parse_filter(matches);
    system_formatters::format_system_info(&system_info, &filter);

    println!();
    println!(
        "{}",
        format!("✓ Collected in {:.2}s", timings.total.as_secs_f64()).bright_black()
    );

    if profile {
        println!("{}", format_profile_report(&timings));
    }

    Ok(())
}

fn parse_filter(matches: &ArgMatches) -> DisplayFilter {
    let show_cpu = matches.get_flag("cpu");
    let show_gpu = matches.get_flag("gpu");
    let show_ram = matches.get_flag("ram");
    let show_mbo = matches.get_flag("mbo");
    let show_network = matches.get_flag("network");
    let show_os = matches.get_flag("os");
    let show_energy = matches.get_flag("energy");

    let none_set = !show_cpu
        && !show_gpu
        && !show_ram
        && !show_mbo
        && !show_network
        && !show_os
        && !show_energy;

    if none_set {
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
    }
}
