use crate::core::system_info::profiler::CollectorTimings;
use crate::core::system_info::types::*;
use crate::core::system_info::{
    battery, cpu, gpu, memory, motherboard, network, os, power, storage,
};
use crate::error::Result;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Number of sections the collector reports — used to size progress bars / spinners.
/// Counts: cpu, motherboard, gpu, network, storage, os, battery, power_plan, memory, npu.
pub const TOTAL_SECTIONS: usize = 10;

/// Collect all system information (no profiling — convenience wrapper).
pub fn collect_system_info() -> Result<SystemInfo> {
    collect_system_info_with_profile().map(|(info, _)| info)
}

/// Collect all system information in parallel with per-section timing data.
pub fn collect_system_info_with_profile() -> Result<(SystemInfo, CollectorTimings)> {
    collect_system_info_with_profile_progress(None)
}

#[inline]
fn tick(progress: &Option<Arc<AtomicUsize>>) {
    if let Some(p) = progress {
        p.fetch_add(1, Ordering::Relaxed);
    }
}

/// Collect all system information in parallel with per-section timing data.
///
/// Stage 1: 8 independent collectors run concurrently via `thread::scope`.
/// Stage 2: `memory` and `npu` need outputs from stage 1 (cpu model + mobo model),
/// so they run after — also concurrently with each other.
///
/// PowerShell invocations are fully process-isolated, so concurrency is safe.
/// `sysinfo` and `wmi` calls used here are also thread-safe per their docs.
///
/// `progress`, when `Some`, is incremented once per completed section so a
/// foreground spinner can render `(done/TOTAL_SECTIONS)`.
pub fn collect_system_info_with_profile_progress(
    progress: Option<Arc<AtomicUsize>>,
) -> Result<(SystemInfo, CollectorTimings)> {
    let total_start = Instant::now();
    let progress = progress; // moved into outer scope; closures borrow &progress
    let progress_ref = &progress;

    // ------ Stage 1 ------
    let s1 = std::thread::scope(|s| {
        let cpu_h = s.spawn(|| {
            let r = timed(|| cpu::collect().unwrap_or_else(|_| cpu::get_fallback()));
            tick(progress_ref);
            r
        });
        let mbo_h = s.spawn(|| {
            let r = timed(|| match motherboard::collect_with_subs() {
                Ok((info, subs)) => (Some(info), subs),
                Err(_) => (None, Vec::new()),
            });
            tick(progress_ref);
            r
        });
        let gpu_h = s.spawn(|| {
            let r = timed(|| gpu::collect().unwrap_or_else(|_| vec![]));
            tick(progress_ref);
            r
        });
        let net_h = s.spawn(|| {
            let r = timed(|| match network::collect_with_subs() {
                Ok((info, subs)) => (info, subs),
                Err(_) => (network::get_fallback(), Vec::new()),
            });
            tick(progress_ref);
            r
        });
        let stor_h = s.spawn(|| {
            let r = timed(|| match storage::collect_with_subs() {
                Ok((info, subs)) => (info, subs),
                Err(_) => (Vec::new(), Vec::new()),
            });
            tick(progress_ref);
            r
        });
        let os_h = s.spawn(|| {
            let r = timed(|| os::collect().unwrap_or_else(|_| os::get_fallback()));
            tick(progress_ref);
            r
        });
        let bat_h = s.spawn(|| {
            let r = timed(|| battery::collect().ok());
            tick(progress_ref);
            r
        });
        let pwr_h = s.spawn(|| {
            let r = timed(|| power::collect().ok());
            tick(progress_ref);
            r
        });

        (
            cpu_h.join().unwrap(),
            mbo_h.join().unwrap(),
            gpu_h.join().unwrap(),
            net_h.join().unwrap(),
            stor_h.join().unwrap(),
            os_h.join().unwrap(),
            bat_h.join().unwrap(),
            pwr_h.join().unwrap(),
        )
    });

    let (cpu_info, cpu_dur) = s1.0;
    let ((motherboard_info, mbo_subs), mbo_dur) = s1.1;
    let (gpu_info, gpu_dur) = s1.2;
    let ((network_info, net_subs), net_dur) = s1.3;
    let ((storage_info, stor_subs), stor_dur) = s1.4;
    let (os_info, os_dur) = s1.5;
    let (battery_info, bat_dur) = s1.6;
    let (power_plan_info, pwr_dur) = s1.7;

    // ------ Stage 2 ------ (depends on cpu + motherboard from stage 1)
    let cpu_model: &str = cpu_info.model.as_str();
    let mbo_model: Option<&str> = motherboard_info.as_ref().and_then(|m| m.product.as_deref());

    let s2 = std::thread::scope(|s| {
        let mem_h = s.spawn(|| {
            let r = timed(|| {
                memory::collect(cpu_model, mbo_model).unwrap_or_else(|_| memory::get_fallback())
            });
            tick(progress_ref);
            r
        });
        let npu_h = s.spawn(|| {
            let r = timed(|| detect_npu(cpu_model));
            tick(progress_ref);
            r
        });

        (mem_h.join().unwrap(), npu_h.join().unwrap())
    });

    let (memory_info, mem_dur) = s2.0;
    let (npu_info, npu_dur) = s2.1;

    let total = total_start.elapsed();

    // ------ Build timings in display order ------
    let mut t = CollectorTimings::new();
    t.sections.push(("cpu".to_string(), cpu_dur));
    t.sections.push(("motherboard".to_string(), mbo_dur));
    t.sections.extend(mbo_subs);
    t.sections.push(("memory".to_string(), mem_dur));
    t.sections.push(("gpu".to_string(), gpu_dur));
    t.sections.push(("network".to_string(), net_dur));
    t.sections.extend(net_subs);
    t.sections.push(("storage".to_string(), stor_dur));
    t.sections.extend(stor_subs);
    t.sections.push(("os".to_string(), os_dur));
    t.sections.push(("npu".to_string(), npu_dur));
    t.sections.push(("battery".to_string(), bat_dur));
    t.sections.push(("power_plan".to_string(), pwr_dur));
    t.total = total;

    let info = SystemInfo {
        cpu: cpu_info,
        memory: memory_info,
        gpu: gpu_info,
        motherboard: motherboard_info,
        network: network_info,
        storage: storage_info,
        os: os_info,
        npu: npu_info,
        battery: battery_info,
        power_plan: power_plan_info,
    };

    Ok((info, t))
}

fn timed<F, T>(f: F) -> (T, Duration)
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    (result, start.elapsed())
}

/// Detect NPU from the CPU model string (no I/O — pure pattern match).
fn detect_npu(cpu_model: &str) -> Option<NpuInfo> {
    let model = cpu_model.to_lowercase();

    // AMD Ryzen AI processors have NPU
    if model.contains("ryzen ai") {
        let tops = if model.contains("ryzen ai 9") || model.contains("ryzen ai 7") {
            Some(50.0) // Ryzen AI 9 HX 370 and AI 7 350/360 have ~50 TOPS
        } else {
            Some(40.0) // Other Ryzen AI models have ~40 TOPS
        };

        return Some(NpuInfo {
            name: "AMD XDNA NPU".to_string(),
            tops,
        });
    }

    // Intel Core Ultra processors have NPU
    if model.contains("core ultra") || model.contains("meteor lake") || model.contains("arrow lake")
    {
        let tops = if model.contains("ultra 9") || model.contains("ultra 7") {
            Some(34.0)
        } else if model.contains("ultra 5") {
            Some(28.0)
        } else {
            Some(30.0)
        };

        return Some(NpuInfo {
            name: "Intel AI Boost NPU".to_string(),
            tops,
        });
    }

    None
}
