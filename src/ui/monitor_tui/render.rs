use ratatui::{
    prelude::*,
    widgets::{BarChart, Block, Borders, Cell, Paragraph, Row, Table},
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
use crate::core::system_monitor::{DiskType, SmartStatus};

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

    // Create main layout - use percentages for more predictable behavior
    let constraints = if has_alerts {
        vec![
            Constraint::Length(3),            // Header with global dashboard
            Constraint::Length(alert_height), // Alerts banner
            Constraint::Percentage(25),       // CPU section
            Constraint::Percentage(20),       // Memory + GPU row
            Constraint::Percentage(15),       // Network + Disk
            Constraint::Percentage(35),       // Processes
            Constraint::Length(1),            // Temperatures
            Constraint::Length(1),            // Footer
        ]
    } else {
        vec![
            Constraint::Length(3),      // Header with global dashboard
            Constraint::Percentage(25), // CPU section
            Constraint::Percentage(20), // Memory + GPU row
            Constraint::Percentage(15), // Network + Disk
            Constraint::Percentage(35), // Processes
            Constraint::Length(1),      // Temperatures
            Constraint::Length(1),      // Footer
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Render sections with proper indexing
    if has_alerts {
        render_global_dashboard(frame, chunks[0], app);
        render_alerts_banner(frame, chunks[1], app);
        render_cpu_section(frame, chunks[2], app);
        render_memory_gpu_section(frame, chunks[3], app);
        render_network_disk_section(frame, chunks[4], app);
        render_processes_section(frame, chunks[5], app);
        render_temperatures_section(frame, chunks[6], app);
        render_footer(frame, chunks[7]);
    } else {
        render_global_dashboard(frame, chunks[0], app);
        render_cpu_section(frame, chunks[1], app);
        render_memory_gpu_section(frame, chunks[2], app);
        render_network_disk_section(frame, chunks[3], app);
        render_processes_section(frame, chunks[4], app);
        render_temperatures_section(frame, chunks[5], app);
        render_footer(frame, chunks[6]);
    }

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
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    // Use smoothed CPU usage for display (if available, otherwise use raw)
    let cpu_usage_display = if app.smoothed_cpu_usage > 0.0 {
        app.smoothed_cpu_usage
    } else {
        cpu.global_usage
    };

    let block = Block::default()
        .title(format!(
            " CPU: {} ({} cores) │ Avg: {:.1}% @ {} MHz │ Load: {:.2} {:.2} {:.2} ",
            cpu.brand,
            cpu.core_count,
            cpu_usage_display,
            avg_freq,
            cpu.load_average.0,
            cpu.load_average.1,
            cpu.load_average.2,
        ))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Only split if we have enough space for sparkline
    if inner.height < 3 || inner.width < 60 {
        // Just show cores, no sparkline (not enough space)
        render_cpu_cores(frame, inner, app);
    } else {
        // Split into left (cores) and right (sparkline)
        // Give more space to sparkline for horizontal rendering
        let cpu_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        // Left: Show per-core usage (with smoothed values)
        render_cpu_cores(frame, cpu_chunks[0], app);

        // Right: CPU History bar chart (last 60 seconds of CPU usage)
        let history_data = app.history.cpu_as_u64();
        if !history_data.is_empty() && cpu_chunks[1].width > 4 {
            // Calculate how many bars can fit
            // Each bar needs: bar_width + bar_gap space
            let inner_width = cpu_chunks[1].width.saturating_sub(2) as usize; // Subtract borders
            let bar_width: u16 = 1;
            let bar_gap: u16 = 1;
            let space_per_bar = bar_width as usize + bar_gap as usize;
            let max_bars = (inner_width / space_per_bar).min(history_data.len());

            // Take the most recent data points
            let start_idx = history_data.len().saturating_sub(max_bars);
            let data_to_show: Vec<(&str, u64)> = history_data[start_idx..]
                .iter()
                .map(|&val| ("", val))
                .collect();

            if !data_to_show.is_empty() {
                let chart = BarChart::default()
                    .block(
                        Block::default()
                            .title("CPU History (60s)")
                            .borders(Borders::ALL),
                    )
                    .direction(Direction::Vertical)
                    .bar_width(bar_width)
                    .bar_gap(bar_gap)
                    .bar_style(Style::default().fg(Color::Cyan))
                    .value_style(Style::default().fg(Color::Black).bg(Color::Cyan))
                    .data(&data_to_show)
                    .max(1000); // CPU usage is 0-100% scaled by 10 (0-1000)

                frame.render_widget(chart, cpu_chunks[1]);
            }
        }
    }
}

/// Render individual CPU cores
fn render_cpu_cores(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    use ratatui::widgets::Gauge;

    let cpu = &app.metrics.cpu;

    // Determine how many cores we can display based on height
    // Make sure we have at least 1 line of space
    let available_height = area.height.max(1) as usize;
    let cores_to_show = available_height.min(cpu.per_core_usage.len());

    if cores_to_show == 0 || cpu.per_core_usage.is_empty() {
        return; // Not enough space or no data
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(1); cores_to_show])
        .split(area);

    for i in 0..cores_to_show {
        // Use smoothed value if available, otherwise fall back to raw value
        let usage = if i < app.smoothed_per_core.len() {
            app.smoothed_per_core[i]
        } else {
            cpu.per_core_usage.get(i).copied().unwrap_or(0.0)
        };

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
    if area.height < 3 {
        return; // Not enough space for memory/gpu section
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let border_style = if app.selected_tab == 1 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
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

    // Only render memory details if we have enough space
    if mem_inner.height < 3 {
        let mem_display = if app.smoothed_memory_usage > 0.0 {
            app.smoothed_memory_usage
        } else {
            mem.usage_percent
        };
        let summary = Paragraph::new(format!("RAM: {:.1}%", mem_display))
            .style(Style::default().fg(Color::White));
        frame.render_widget(summary, mem_inner);
    } else {
        let mem_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Used RAM
                Constraint::Length(1), // Cache/Buffers
                Constraint::Length(2), // SWAP
            ])
            .split(mem_inner);

        // Real used RAM (excluding cache) - use smoothed value if available
        let mem_display = if app.smoothed_memory_usage > 0.0 {
            app.smoothed_memory_usage
        } else {
            mem.usage_percent
        };
        let ram_text = format!(
            "Used:  {} / {} ({:.1}%)",
            format_size(mem.used_bytes),
            format_size(mem.total_bytes),
            mem_display
        );
        let ram_gauge = colored_gauge(mem_display as f64, &ram_text);
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
    }

    // GPU
    let gpu_block = Block::default()
        .title(" GPU ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let gpu_inner = gpu_block.inner(chunks[1]);
    frame.render_widget(gpu_block, chunks[1]);

    if let Some(ref gpu) = app.metrics.gpu {
        // Only render full GPU details if we have enough space
        if gpu_inner.height < 3 {
            let gpu_display = if app.smoothed_gpu_usage > 0.0 {
                app.smoothed_gpu_usage
            } else {
                gpu.utilization_percent as f32
            };
            let summary = Paragraph::new(format!("GPU: {:.0}%", gpu_display))
                .style(Style::default().fg(Color::Cyan));
            frame.render_widget(summary, gpu_inner);
        } else {
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

            // Use smoothed GPU usage if available
            let gpu_display = if app.smoothed_gpu_usage > 0.0 {
                app.smoothed_gpu_usage
            } else {
                gpu.utilization_percent as f32
            };
            let usage_text = format!(
                "Use: {:.0}% │ VRAM: {} / {}",
                gpu_display,
                format_size(gpu.memory_used_bytes),
                format_size(gpu.memory_total_bytes)
            );
            let usage_gauge = colored_gauge(gpu_display as f64, &usage_text);
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
        }
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
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
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

    if net_inner.height == 0 || net_inner.width == 0 {
        return; // No space to render network/disk tables
    }

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

    if !net_rows.is_empty() {
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
    } else {
        let no_data =
            Paragraph::new("No network interfaces").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_data, net_inner);
    }

    // Disks - Enhanced display with SMART data
    let disk_block = Block::default()
        .title(" Storage Devices ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let disk_inner = disk_block.inner(chunks[1]);
    frame.render_widget(disk_block, chunks[1]);

    if app.metrics.disks.is_empty() {
        let no_data =
            Paragraph::new("No disks detected").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_data, disk_inner);
    } else {
        // Render up to 3 disks with detailed information
        let available_height = disk_inner.height;
        let lines_per_disk = 3; // Each disk takes 3 lines
        let max_disks = ((available_height / lines_per_disk) as usize).min(3);

        let mut disk_lines = Vec::new();

        for (idx, disk) in app.metrics.disks.iter().take(max_disks).enumerate() {
            // Line 1: Mount point, Type icon, SMART status icon, Temperature
            let type_icon = match disk.disk_type.as_ref() {
                Some(DiskType::NVMe) => "⚡",
                Some(DiskType::SSD) => "💿",
                Some(DiskType::HDD) => "💾",
                _ => "📀",
            };

            let smart_icon = match disk.smart_status.as_ref() {
                Some(SmartStatus::Healthy) => Span::styled("✓", Style::default().fg(Color::Green)),
                Some(SmartStatus::Warning) => Span::styled("⚠", Style::default().fg(Color::Yellow)),
                Some(SmartStatus::Critical) => Span::styled("✗", Style::default().fg(Color::Red)),
                _ => Span::styled("?", Style::default().fg(Color::DarkGray)),
            };

            let temp_text = if let Some(temp) = disk.temperature_celsius {
                let temp_color = if temp > 60 {
                    Color::Red
                } else if temp > 45 {
                    Color::Yellow
                } else {
                    Color::Green
                };
                Span::styled(format!(" {}°C", temp), Style::default().fg(temp_color))
            } else {
                Span::raw("")
            };

            let line1 = Line::from(vec![
                Span::raw(format!("{} ", type_icon)),
                Span::styled(
                    disk.mount_point.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                smart_icon,
                temp_text,
            ]);

            // Line 2: Manufacturer/Model + Interface speed
            let model_text = match (&disk.manufacturer, &disk.model) {
                (Some(mfr), Some(model)) => format!("{} {}", mfr, model),
                (Some(mfr), None) => mfr.clone(),
                (None, Some(model)) => model.clone(),
                (None, None) => disk.fs_type.clone(),
            };

            let interface_text = if let Some(ref interface) = disk.interface_speed {
                format!(" • {}", interface)
            } else {
                String::new()
            };

            let line2 = Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    model_text,
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
                Span::styled(interface_text, Style::default().fg(Color::Cyan)),
            ]);

            // Line 3: Usage bar + size + power-on hours
            let used = disk.total_bytes - disk.available_bytes;
            let usage_pct = disk.usage_percent;

            let bar_width = disk_inner.width.saturating_sub(40).max(10) as usize;
            let filled = ((bar_width as f32 * usage_pct / 100.0) as usize).min(bar_width);
            let empty = bar_width.saturating_sub(filled);

            let bar_color = if usage_pct > 90.0 {
                Color::Red
            } else if usage_pct > 75.0 {
                Color::Yellow
            } else {
                Color::Green
            };

            let mut bar_spans = vec![Span::raw("  [")];
            if filled > 0 {
                bar_spans.push(Span::styled(
                    "█".repeat(filled),
                    Style::default().fg(bar_color),
                ));
            }
            if empty > 0 {
                bar_spans.push(Span::styled(
                    "░".repeat(empty),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            bar_spans.push(Span::raw("] "));
            bar_spans.push(Span::styled(
                format!("{:.1}% ", usage_pct),
                Style::default().fg(bar_color).add_modifier(Modifier::BOLD),
            ));
            bar_spans.push(Span::raw(format!(
                "{}/{}",
                format_size(used),
                format_size(disk.total_bytes)
            )));

            if let Some(hours) = disk.power_on_hours {
                let days = hours / 24;
                bar_spans.push(Span::styled(
                    format!(" • {}d", days),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let line3 = Line::from(bar_spans);

            disk_lines.push(line1);
            disk_lines.push(line2);
            disk_lines.push(line3);

            // Add separator between disks (except after last one)
            if idx < max_disks - 1 && idx < app.metrics.disks.len() - 1 {
                disk_lines.push(Line::from(""));
            }
        }

        let disk_paragraph = Paragraph::new(disk_lines);
        frame.render_widget(disk_paragraph, disk_inner);
    }
}

fn render_processes_section(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    use crate::core::system_monitor::{build_process_tree, flatten_tree, format_tree_indent};

    let mode_str = if app.show_process_tree {
        "Tree"
    } else {
        "List"
    };

    let border_style = if app.selected_tab == 3 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
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

    // Check if we have space to render
    if inner.height < 2 {
        return; // Not enough space for header + at least one row
    }

    let header = Row::new(vec![
        Cell::from("PID").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("CPU %").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Memory").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .height(1);

    let rows: Vec<Row> = if app.metrics.top_processes.is_empty() {
        vec![] // No processes to show
    } else if app.show_process_tree {
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
    if area.height == 0 {
        return; // No space to render
    }

    let temps: String = if app.metrics.temperatures.is_empty() {
        "No temperature sensors detected".to_string()
    } else {
        app.metrics
            .temperatures
            .iter()
            .take(6)
            .map(|t| {
                let _color = temp_color(t.current_celsius);
                format!("{}: {:.0}°C", t.label, t.current_celsius)
            })
            .collect::<Vec<_>>()
            .join(" │ ")
    };

    let text = if temps.is_empty() {
        " Temperatures: - ".to_string()
    } else {
        format!(" Temperatures: {} ", temps)
    };

    let para = Paragraph::new(text).style(Style::default().fg(Color::White));
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
