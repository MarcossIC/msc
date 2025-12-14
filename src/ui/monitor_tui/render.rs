use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Paragraph, Row, Sparkline, Table},
};

// Assuming format_size is available in crate::ui::formatters or use humansize directly if needed.
// The plan references crate::ui::formatters::format_size.
// If it implies I should adding it, I'll check.
// For now I'll use humansize crate directly if formatters is not available or assume it is.
// I will check ui/mod.rs later. For now I'll use a local helper or humansize.
// Update: Plan says `use crate::ui::formatters::format_size;`. I'll assume it exists or I might fail compiling.
// I'll check ui/mod.rs in next step.
// For safety, I'll implement a `format_size` helper here if I can't check.
// Actually, looking at previous conversation history, humansize is a dependency.
// I'll assume `crate::ui::formatters::format_size` exists as per plan, if not I'll fix it.

use humansize::{format_size as human_format_size, DECIMAL};

fn format_size(bytes: u64) -> String {
    human_format_size(bytes, DECIMAL)
}

use super::app::MonitorApp;
use super::widgets::{colored_gauge, temp_color};

/// Main render function
pub fn render_ui(frame: &mut Frame, app: &MonitorApp) {
    let area = frame.area();

    // Determine if we need to show alerts banner
    let has_alerts = !app.alerts.is_empty();
    let alert_height = if has_alerts {
        // 1 line per alert + 2 for borders
        (app.alerts.len().min(3) + 2) as u16
    } else {
        0
    };

    // Create main layout
    let constraints = if has_alerts {
        vec![
            Constraint::Length(3),            // Header with global dashboard
            Constraint::Length(alert_height), // Alerts banner
            Constraint::Length(8),            // CPU
            Constraint::Length(7),            // Memory + GPU row
            Constraint::Length(5),            // Network + Disk
            Constraint::Min(8),               // Processes
            Constraint::Length(2),            // Temperatures
            Constraint::Length(1),            // Footer
        ]
    } else {
        vec![
            Constraint::Length(3), // Header with global dashboard
            Constraint::Length(8), // CPU
            Constraint::Length(7), // Memory + GPU row
            Constraint::Length(5), // Network + Disk
            Constraint::Min(8),    // Processes
            Constraint::Length(2), // Temperatures
            Constraint::Length(1), // Footer
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut chunk_idx = 0;
    render_global_dashboard(frame, chunks[chunk_idx], app);
    chunk_idx += 1;

    if has_alerts {
        render_alerts_banner(frame, chunks[chunk_idx], app);
        chunk_idx += 1;
    }

    render_cpu_section(frame, chunks[chunk_idx], app);
    chunk_idx += 1;
    render_memory_gpu_section(frame, chunks[chunk_idx], app);
    chunk_idx += 1;
    render_network_disk_section(frame, chunks[chunk_idx], app);
    chunk_idx += 1;
    render_processes_section(frame, chunks[chunk_idx], app);
    chunk_idx += 1;
    render_temperatures_section(frame, chunks[chunk_idx], app);
    chunk_idx += 1;
    render_footer(frame, chunks[chunk_idx]);

    // Render help overlay if active
    if app.show_help {
        render_help_overlay(frame, area);
    }
}

/// Render global dashboard with system-wide metrics
fn render_global_dashboard(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    use crate::core::system_monitor::PowerSource;

    let global = &app.metrics.global;

    // Format uptime
    let uptime_str = format_duration(global.uptime_secs);

    // Format load average
    let load_str = format!(
        "{:.2} {:.2} {:.2}",
        app.metrics.cpu.load_average.0,
        app.metrics.cpu.load_average.1,
        app.metrics.cpu.load_average.2
    );

    // Format power source and battery
    let power_str = match global.power_source {
        PowerSource::AC => "⚡ AC".to_string(),
        PowerSource::Battery => {
            if let Some(pct) = global.battery_percent {
                let icon = if pct > 80.0 {
                    "🔋"
                } else if pct > 20.0 {
                    "🔌"
                } else {
                    "🪫"
                };
                format!("{} {:.0}%", icon, pct)
            } else {
                "🔋 Battery".to_string()
            }
        }
        PowerSource::Unknown => "? Unknown".to_string(),
    };

    // Determine color based on battery level
    let power_color = if global.power_source == PowerSource::Battery {
        if let Some(pct) = global.battery_percent {
            if pct < 20.0 {
                Color::Red
            } else if pct < 50.0 {
                Color::Yellow
            } else {
                Color::Green
            }
        } else {
            Color::White
        }
    } else {
        Color::Cyan
    };

    let title = format!(
        " {} │ Uptime: {} │ Load: {} │ {} │ Refresh: {}ms ",
        global.hostname, uptime_str, load_str, power_str, app.interval_ms
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(power_color));

    frame.render_widget(block, area);
}

/// Format duration in seconds to human-readable format
fn format_duration(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;

    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

/// Render alerts banner
fn render_alerts_banner(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    use crate::core::system_monitor::AlertSeverity;

    let block = Block::default()
        .title(" ⚠ ALERTS ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Show up to 3 most severe alerts
    let mut alerts_to_show: Vec<_> = app.alerts.iter().collect();
    alerts_to_show.sort_by_key(|a| match a.severity {
        AlertSeverity::Critical => 0,
        AlertSeverity::Warning => 1,
        AlertSeverity::Info => 2,
    });
    alerts_to_show.truncate(3);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(1); alerts_to_show.len()])
        .split(inner);

    for (i, alert) in alerts_to_show.iter().enumerate() {
        let (icon, color) = match alert.severity {
            AlertSeverity::Critical => ("🔴", Color::Red),
            AlertSeverity::Warning => ("⚠ ", Color::Yellow),
            AlertSeverity::Info => ("🔵", Color::Cyan),
        };

        let text = Paragraph::new(format!("{} {}", icon, alert.message))
            .style(Style::default().fg(color).add_modifier(Modifier::BOLD));

        frame.render_widget(text, layout[i]);
    }
}

fn render_cpu_section(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    let cpu = &app.metrics.cpu;

    // Calculate average frequency
    let avg_freq = if !cpu.frequencies_mhz.is_empty() {
        cpu.frequencies_mhz.iter().sum::<u64>() / cpu.frequencies_mhz.len() as u64
    } else {
        0
    };

    let border_style = if app.selected_tab == 0 {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let block = Block::default()
        .title(format!(
            " CPU: {} ({} cores) │ Avg: {:.1}% @ {} MHz │ Load: {:.2} {:.2} {:.2} ",
            cpu.brand,
            cpu.core_count,
            cpu.global_usage,
            avg_freq,
            cpu.load_average.0,
            cpu.load_average.1,
            cpu.load_average.2,
        ))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split into left (cores) and right (sparkline)
    let cpu_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
        .split(inner);

    // Left: Show per-core usage
    render_cpu_cores(frame, cpu_chunks[0], cpu);

    // Right: CPU History sparkline
    let sparkline = Sparkline::default()
        .block(Block::default().title("History").borders(Borders::ALL))
        .data(app.history.cpu_as_u64())
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(sparkline, cpu_chunks[1]);
}

/// Render individual CPU cores
fn render_cpu_cores(frame: &mut Frame, area: Rect, cpu: &crate::core::system_monitor::CpuMetrics) {
    use ratatui::widgets::Gauge;

    // Determine how many cores we can display based on height
    let available_height = area.height as usize;
    let cores_to_show = (available_height).min(cpu.per_core_usage.len());

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(1); cores_to_show])
        .split(area);

    for (i, &usage) in cpu.per_core_usage.iter().take(cores_to_show).enumerate() {
        let freq = cpu.frequencies_mhz.get(i).copied().unwrap_or(0);

        // Determine color based on usage
        let color = if usage > 90.0 {
            Color::Red
        } else if usage > 75.0 {
            Color::Yellow
        } else if usage > 50.0 {
            Color::Cyan
        } else {
            Color::Green
        };

        // Add "HOT" indicator for high usage
        let hot_indicator = if usage > 90.0 { " ⚠ HOT" } else { "" };

        let label = format!(
            "C{:02} [{:>5.1}%] @ {:>4} MHz{}",
            i, usage, freq, hot_indicator
        );

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(color))
            .label(label)
            .ratio((usage / 100.0).min(1.0) as f64);

        frame.render_widget(gauge, layout[i]);
    }
}

fn render_memory_gpu_section(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let border_style = if app.selected_tab == 1 {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    // Memory
    let mem = &app.metrics.memory;
    let mem_block = Block::default()
        .title(" Memory ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let mem_inner = mem_block.inner(chunks[0]);
    frame.render_widget(mem_block, chunks[0]);

    let mem_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Used RAM
            Constraint::Length(1), // Cache/Buffers
            Constraint::Length(2), // SWAP
        ])
        .split(mem_inner);

    // Real used RAM (excluding cache)
    let ram_text = format!(
        "Used:  {} / {} ({:.1}%)",
        format_size(mem.used_bytes),
        format_size(mem.total_bytes),
        mem.usage_percent
    );
    let ram_gauge = colored_gauge(mem.usage_percent as f64, &ram_text);
    frame.render_widget(ram_gauge, mem_layout[0]);

    // Cache/Buffers indicator
    let cache_percent = if mem.total_bytes > 0 {
        (mem.cache_buffers_bytes as f32 / mem.total_bytes as f32) * 100.0
    } else {
        0.0
    };
    let cache_text = Paragraph::new(format!(
        "Cache: {} ({:.1}%)",
        format_size(mem.cache_buffers_bytes),
        cache_percent
    ))
    .style(Style::default().fg(Color::Blue));
    frame.render_widget(cache_text, mem_layout[1]);

    // SWAP
    let swap_text = format!(
        "Swap:  {} / {} ({:.1}%)",
        format_size(mem.swap_used_bytes),
        format_size(mem.swap_total_bytes),
        mem.swap_percent
    );
    let swap_gauge = colored_gauge(mem.swap_percent as f64, &swap_text);
    frame.render_widget(swap_gauge, mem_layout[2]);

    // GPU
    let gpu_block = Block::default()
        .title(" GPU ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let gpu_inner = gpu_block.inner(chunks[1]);
    frame.render_widget(gpu_block, chunks[1]);

    if let Some(ref gpu) = app.metrics.gpu {
        let gpu_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Length(1),
            ])
            .split(gpu_inner);

        let name = Paragraph::new(gpu.name.clone()).style(Style::default().fg(Color::Cyan));
        frame.render_widget(name, gpu_layout[0]);

        let usage_text = format!(
            "Use: {}% │ VRAM: {} / {}",
            gpu.utilization_percent,
            format_size(gpu.memory_used_bytes),
            format_size(gpu.memory_total_bytes)
        );
        let usage_gauge = colored_gauge(gpu.utilization_percent as f64, &usage_text);
        frame.render_widget(usage_gauge, gpu_layout[1]);

        let details = format!(
            "Temp: {}°C │ Fan: {}% │ Power: {}W/{}W",
            gpu.temperature_celsius
                .map(|t| t.to_string())
                .unwrap_or("N/A".into()),
            gpu.fan_speed_percent
                .map(|f| f.to_string())
                .unwrap_or("N/A".into()),
            gpu.power_draw_watts
                .map(|p| p.to_string())
                .unwrap_or("N/A".into()),
            gpu.power_limit_watts
                .map(|p| p.to_string())
                .unwrap_or("N/A".into()),
        );
        let details_para = Paragraph::new(details).style(Style::default().fg(Color::White));
        frame.render_widget(details_para, gpu_layout[2]);
    } else {
        let no_gpu = Paragraph::new("No GPU detected").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_gpu, gpu_inner);
    }
}

fn render_network_disk_section(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let border_style = if app.selected_tab == 2 {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    // Network
    let net_block = Block::default()
        .title(" Network ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let net_inner = net_block.inner(chunks[0]);
    frame.render_widget(net_block, chunks[0]);

    let net_rows: Vec<Row> = app
        .metrics
        .network
        .iter()
        .take(3)
        .map(|net| {
            // Determine if there are errors or drops
            let has_errors = net.rx_errors > 0 || net.tx_errors > 0;
            let error_style = if has_errors {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let errors_str = if has_errors {
                format!("⚠ {}", net.rx_errors + net.tx_errors)
            } else {
                "✓".to_string()
            };

            Row::new(vec![
                Cell::from(net.interface.clone()),
                Cell::from(format!("↓ {}/s", format_size(net.rx_bytes_per_sec))),
                Cell::from(format!("↑ {}/s", format_size(net.tx_bytes_per_sec))),
                Cell::from(errors_str).style(error_style),
            ])
        })
        .collect();

    let net_table = Table::new(
        net_rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(28),
            Constraint::Percentage(28),
            Constraint::Percentage(14),
        ],
    );
    frame.render_widget(net_table, net_inner);

    // Disks
    let disk_block = Block::default()
        .title(" Disks ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let disk_inner = disk_block.inner(chunks[1]);
    frame.render_widget(disk_block, chunks[1]);

    let disk_rows: Vec<Row> = app
        .metrics
        .disks
        .iter()
        .take(3)
        .map(|disk| {
            Row::new(vec![
                Cell::from(disk.mount_point.clone()),
                Cell::from(format!("{:.1}%", disk.usage_percent)),
                Cell::from(format!(
                    "{}/{}",
                    format_size(disk.total_bytes - disk.available_bytes),
                    format_size(disk.total_bytes)
                )),
            ])
        })
        .collect();

    let disk_table = Table::new(
        disk_rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(40),
        ],
    );
    frame.render_widget(disk_table, disk_inner);
}

fn render_processes_section(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    use crate::core::system_monitor::{build_process_tree, flatten_tree, format_tree_indent};

    let mode_str = if app.show_process_tree {
        "Tree"
    } else {
        "List"
    };

    let border_style = if app.selected_tab == 3 {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let block = Block::default()
        .title(format!(
            " Processes ({}) [t:toggle s:sort ↑↓:nav] ",
            mode_str
        ))
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let header = Row::new(vec![
        Cell::from("PID").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("CPU %").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Memory").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .height(1);

    let rows: Vec<Row> = if app.show_process_tree {
        // Build and flatten tree
        let tree = build_process_tree(&app.metrics.top_processes);
        let flattened = flatten_tree(&tree);

        flattened
            .iter()
            .enumerate()
            .map(|(i, flat_proc)| {
                let indent = format_tree_indent(flat_proc);
                let proc = &flat_proc.process;

                // Highlight selected row
                let style = if i == app.selected_process_index {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(proc.pid.to_string()),
                    Cell::from(format!("{}{}", indent, proc.name)),
                    Cell::from(format!("{:.1}%", proc.cpu_usage_percent)),
                    Cell::from(format_size(proc.memory_bytes)),
                ])
                .style(style)
            })
            .collect()
    } else {
        // Flat list view
        app.metrics
            .top_processes
            .iter()
            .enumerate()
            .map(|(i, proc)| {
                let style = if i == app.selected_process_index {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(proc.pid.to_string()),
                    Cell::from(proc.name.clone()),
                    Cell::from(format!("{:.1}%", proc.cpu_usage_percent)),
                    Cell::from(format_size(proc.memory_bytes)),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Percentage(45),
            Constraint::Length(10),
            Constraint::Length(12),
        ],
    )
    .header(header);

    frame.render_widget(table, inner);
}

fn render_temperatures_section(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    let temps: String = app
        .metrics
        .temperatures
        .iter()
        .take(6)
        .map(|t| {
            let _color = temp_color(t.current_celsius);
            format!("{}: {:.0}°C", t.label, t.current_celsius)
        })
        .collect::<Vec<_>>()
        .join(" │ ");

    let para = Paragraph::new(format!(" Temperatures: {} ", temps))
        .style(Style::default().fg(Color::White));
    frame.render_widget(para, area);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let help = " q: Quit │ ?: Help │ Tab: Switch section │ s: Sort processes ";
    let para = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(para, area);
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let help_text = r#"
    MSC System Monitor - Help

    Keyboard Shortcuts:
    ─────────────────────────────────────
    q / Esc     Quit the application
    ? / h       Toggle this help screen
    Tab         Next section
    Shift+Tab   Previous section
    s           Toggle process sort (CPU/Memory)

    Press any key to close this help
    "#;

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::DarkGray));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .alignment(Alignment::Left);

    // Center the help popup
    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(paragraph, popup_area);
}

/// Helper function to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
