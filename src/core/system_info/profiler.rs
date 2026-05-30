use colored::*;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct CollectorTimings {
    pub sections: Vec<(String, Duration)>,
    pub total: Duration,
}

impl CollectorTimings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record<F, T>(&mut self, name: &str, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let start = Instant::now();
        let result = f();
        let elapsed = start.elapsed();
        self.sections.push((name.to_string(), elapsed));
        result
    }

    /// Record a parent section plus its sub-section timings.
    /// Sub names should be in the form `"parent.child"` for proper rendering.
    pub fn record_with_subs<F, T>(&mut self, name: &str, f: F) -> T
    where
        F: FnOnce() -> (T, Vec<(String, Duration)>),
    {
        let start = Instant::now();
        let (result, subs) = f();
        let elapsed = start.elapsed();
        self.sections.push((name.to_string(), elapsed));
        for sub in subs {
            self.sections.push(sub);
        }
        result
    }
}

pub fn format_profile_report(timings: &CollectorTimings) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "\n{}\n",
        "PROFILE REPORT — sys info".bold().bright_yellow()
    ));
    out.push_str(&format!("{}\n", "=".repeat(70)));
    out.push_str(&format!(
        "  {:<38} {:>12} {:>12}\n",
        "Section".bold(),
        "Time".bold(),
        "% of wall".bold()
    ));
    out.push_str(&format!("{}\n", "-".repeat(70)));

    let total_secs = timings.total.as_secs_f64().max(0.000_001);

    // Preserve insertion order — parent first, then its subs.
    for (name, duration) in &timings.sections {
        let secs = duration.as_secs_f64();
        let pct = (secs / total_secs) * 100.0;
        let time_str = format_duration(*duration);

        let is_sub = name.contains('.');
        let display_name = if is_sub {
            let child = name.split_once('.').map(|(_, c)| c).unwrap_or(name);
            format!("  └─ {}", child)
        } else {
            name.clone()
        };

        let colored_pct = if pct >= 25.0 {
            format!("{:>10.1}%", pct).red().to_string()
        } else if pct >= 10.0 {
            format!("{:>10.1}%", pct).yellow().to_string()
        } else {
            format!("{:>10.1}%", pct).green().to_string()
        };

        out.push_str(&format!(
            "  {:<38} {:>12} {:>12}\n",
            display_name, time_str, colored_pct
        ));
    }

    out.push_str(&format!("{}\n", "-".repeat(70)));

    // Top-level sections only — used for both serial sum and insights.
    let top_level: Vec<_> = timings
        .sections
        .iter()
        .filter(|(n, _)| !n.contains('.'))
        .cloned()
        .collect();

    let serial_sum: Duration = top_level.iter().map(|(_, d)| *d).sum();
    let speedup = if timings.total.as_secs_f64() > 0.0 {
        serial_sum.as_secs_f64() / timings.total.as_secs_f64()
    } else {
        1.0
    };

    out.push_str(&format!(
        "  {:<38} {:>12}  (wall-clock)\n",
        "TOTAL".bold(),
        format_duration(timings.total).bold()
    ));
    out.push_str(&format!(
        "  {:<38} {:>12}  (sum of work — would be serial total)\n",
        "Σ work".bold(),
        format_duration(serial_sum).bold()
    ));
    out.push_str(&format!(
        "  {:<38} {:>12.2}×\n",
        "Speedup (parallelization gain)".bold(),
        speedup
    ));
    out.push_str(&format!("{}\n", "=".repeat(70)));

    let tips = generate_tips(&top_level, timings.total);
    if !tips.is_empty() {
        out.push_str(&format!("\n{}\n", "INSIGHTS".bold().bright_cyan()));
        for tip in tips {
            out.push_str(&format!("  • {}\n", tip));
        }
    }

    out
}

fn format_duration(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms >= 1000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else if ms >= 1.0 {
        format!("{:.1}ms", ms)
    } else {
        format!("{:.0}µs", d.as_micros())
    }
}

fn generate_tips(top_level: &[(String, Duration)], total: Duration) -> Vec<String> {
    let mut tips = Vec::new();
    let total_ms = total.as_secs_f64() * 1000.0;

    let mut sorted = top_level.to_vec();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let cpu_calls: Vec<_> = sorted
        .iter()
        .filter(|(n, _)| n.starts_with("cpu"))
        .collect();
    if cpu_calls.len() > 1 {
        let total_cpu_ms: f64 = cpu_calls
            .iter()
            .map(|(_, d)| d.as_secs_f64() * 1000.0)
            .sum();
        tips.push(format!(
            "{} CPU is collected {} times (total {:.1}ms wasted on duplicate work)",
            "DUPLICATION:".red().bold(),
            cpu_calls.len(),
            total_cpu_ms - cpu_calls[0].1.as_secs_f64() * 1000.0
        ));
    }

    let mbo_calls: Vec<_> = sorted
        .iter()
        .filter(|(n, _)| n.starts_with("motherboard"))
        .collect();
    if mbo_calls.len() > 1 {
        tips.push(format!(
            "{} Motherboard is collected {} times — should be cached",
            "DUPLICATION:".red().bold(),
            mbo_calls.len()
        ));
    }

    if let Some((name, dur)) = sorted.first() {
        let pct = (dur.as_secs_f64() * 1000.0) / total_ms * 100.0;
        if pct >= 25.0 {
            tips.push(format!(
                "{} '{}' takes {:.0}% of total time — prime optimization target",
                "HOTSPOT:".yellow().bold(),
                name,
                pct
            ));
        }
    }

    if total_ms >= 5000.0 {
        tips.push(format!(
            "{} total time {} is above 5s — parallelization + WMI direct calls would cut this drastically",
            "SLOW:".red().bold(),
            format_duration(total)
        ));
    }

    tips
}
