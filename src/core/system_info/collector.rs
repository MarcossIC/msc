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

/// Join a scoped collector thread, recovering from an internal panic.
///
/// Each collector thread already turns its own `Err` into a fallback, so
/// `join()` yields `Err` *only* when the collector itself panicked — a stray
/// `unwrap`/index deep in a WMI, raw-cpuid, or Windows API path. Rather than
/// let that panic abort the whole `sys` command (the project's flagship
/// diagnostic), we log it and substitute the same fallback the thread would
/// have used. Consuming the `Err` here is also what stops `thread::scope` from
/// re-panicking: `join()` `.take()`s the packet result, so `Packet::drop` never
/// flags the scope as panicked.
fn recover<T>(result: std::thread::Result<T>, section: &str, fallback: impl FnOnce() -> T) -> T {
    result.unwrap_or_else(|_| {
        log::warn!("system_info: '{section}' collector panicked; using fallback");
        fallback()
    })
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
    let progress_ref = &progress; // closures borrow the param directly

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
        // PCIe link reader: its OWN Stage 1 thread, parallel to storage, so its
        // ~1-4ms stays off the storage critical path. No `tick` — it's not a
        // user-visible section, just an enrichment merged in after the join.
        let link_h = s.spawn(|| storage::disk_link_map());

        (
            recover(cpu_h.join(), "cpu", || {
                (cpu::get_fallback(), Duration::ZERO)
            }),
            recover(mbo_h.join(), "motherboard", || {
                ((None, Vec::new()), Duration::ZERO)
            }),
            recover(gpu_h.join(), "gpu", || (vec![], Duration::ZERO)),
            recover(net_h.join(), "network", || {
                ((network::get_fallback(), Vec::new()), Duration::ZERO)
            }),
            recover(stor_h.join(), "storage", || {
                ((Vec::new(), Vec::new()), Duration::ZERO)
            }),
            recover(os_h.join(), "os", || (os::get_fallback(), Duration::ZERO)),
            recover(bat_h.join(), "battery", || (None, Duration::ZERO)),
            recover(pwr_h.join(), "power_plan", || (None, Duration::ZERO)),
            recover(link_h.join(), "disk_links", std::collections::HashMap::new),
        )
    });

    let (cpu_info, cpu_dur) = s1.0;
    let ((motherboard_info, mbo_subs), mbo_dur) = s1.1;
    let (gpu_info, gpu_dur) = s1.2;
    let ((network_info, net_subs), net_dur) = s1.3;
    let ((mut storage_info, stor_subs), stor_dur) = s1.4;
    let (os_info, os_dur) = s1.5;
    let (battery_info, bat_dur) = s1.6;
    let (power_plan_info, pwr_dur) = s1.7;
    let disk_link_map = s1.8;

    // Assign each disk its REAL PCIe link by physical drive number. Only disks
    // the reader resolved are touched; others keep interface_speed = None (we
    // never fabricate a generation). Different NVMe can land on different gen/width.
    for disk in &mut storage_info {
        if let Some(n) = disk.disk_number {
            if let Some(speed) = disk_link_map.get(&n) {
                disk.interface_speed = Some(speed.clone());
            }
        }
    }

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

        (
            recover(mem_h.join(), "memory", || {
                (memory::get_fallback(), Duration::ZERO)
            }),
            recover(npu_h.join(), "npu", || (None, Duration::ZERO)),
        )
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

#[cfg(test)]
mod tests {
    use super::recover;
    use std::time::Duration;

    #[test]
    fn recover_passes_value_through_on_ok() {
        let v = recover(Ok((42u32, Duration::from_millis(5))), "cpu", || {
            (0, Duration::ZERO)
        });
        assert_eq!(v, (42, Duration::from_millis(5)));
    }

    #[test]
    fn recover_returns_fallback_on_err() {
        let err: std::thread::Result<(u32, Duration)> = Err(Box::new("boom"));
        let v = recover(err, "cpu", || (99, Duration::ZERO));
        assert_eq!(v, (99, Duration::ZERO));
    }

    #[test]
    fn panicking_scoped_thread_is_recovered_not_propagated() {
        // The decisive test: a scoped collector that panics must NOT crash the
        // scope. `join()` `.take()`s the packet result, so consuming the `Err`
        // here stops `thread::scope` from re-panicking with
        // "a scoped thread panicked". If the `.take()` reasoning were wrong,
        // this test would itself panic instead of returning the fallback.
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {})); // silence child-thread backtrace
        let out = std::thread::scope(|s| {
            let h = s.spawn(|| -> u32 { panic!("collector exploded") });
            recover(h.join(), "panicky", || 7)
        });
        std::panic::set_hook(prev);
        assert_eq!(out, 7);
    }
}
