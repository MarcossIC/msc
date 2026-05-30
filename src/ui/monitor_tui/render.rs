use ratatui::{
    prelude::*,
    widgets::{BarChart, Block, Borders, Cell, Paragraph, Row, Table},
};

use super::app::MonitorApp;
use super::render_cache::{BAR_EMPTY, BAR_FILLED, BLOCK_CHAR_BYTES};
use super::widgets::colored_gauge;

/// Minimum terminal dimensions for a usable dashboard.
const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 24;

/// Main render function
pub fn render_ui(frame: &mut Frame, app: &mut MonitorApp) {
    let area = frame.area();

    // Show a clear message instead of an empty/broken layout
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small_message(frame, area);
        return;
    }

    let has_alerts = !app.alerts.is_empty();
    let alert_height = if has_alerts {
        (app.alerts.len().min(3) + 2) as u16
    } else {
        0
    };

    let constraints = if has_alerts {
        vec![
            Constraint::Length(3),
            Constraint::Length(alert_height),
            Constraint::Fill(5),
            Constraint::Fill(4),
            Constraint::Fill(3),
            Constraint::Fill(7),
            Constraint::Length(1),
            Constraint::Length(1),
        ]
    } else {
        vec![
            Constraint::Length(3),
            Constraint::Fill(5),
            Constraint::Fill(4),
            Constraint::Fill(3),
            Constraint::Fill(7),
            Constraint::Length(1),
            Constraint::Length(1),
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

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

    if app.show_help {
        render_help_overlay(frame, area);
    }
}

/// Render global dashboard — all strings pre-computed in RenderCache
fn render_global_dashboard(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    let block = Block::default()
        .title(app.render_cache.global_title.as_str())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.render_cache.power_color));
    frame.render_widget(block, area);
}

/// Render alerts banner (not cached — alerts are infrequent)
fn render_alerts_banner(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    use crate::core::system_monitor::AlertSeverity;

    let block = Block::default()
        .title(" ⚠ ALERTS ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(area);
    frame.render_widget(block, area);

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
    let border_style = if app.selected_tab == 0 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let block = Block::default()
        .title(app.render_cache.cpu_title.as_str())
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 3 || inner.width < 60 {
        render_cpu_cores(frame, inner, app);
    } else {
        let cpu_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        render_cpu_cores(frame, cpu_chunks[0], app);

        // CPU History bar chart
        let history_data = app.history.cpu_as_u64();
        let chart_block = Block::default()
            .title("CPU History (60s)")
            .borders(Borders::ALL);
        let chart_inner = chart_block.inner(cpu_chunks[1]);

        if !history_data.is_empty() && chart_inner.width > 2 {
            let bar_width: u16 = 1;
            let bar_gap: u16 = 0;
            let space_per_bar = (bar_width + bar_gap).max(1) as usize;
            let max_bars = (chart_inner.width as usize / space_per_bar).min(history_data.len());

            let start_idx = history_data.len().saturating_sub(max_bars);
            let data_to_show: Vec<(&str, u64)> = history_data[start_idx..]
                .iter()
                .map(|&val| ("", val))
                .collect();

            if !data_to_show.is_empty() {
                let chart = BarChart::default()
                    .block(chart_block)
                    .direction(Direction::Vertical)
                    .bar_width(bar_width)
                    .bar_gap(bar_gap)
                    .bar_style(Style::default().fg(Color::Cyan))
                    .value_style(Style::default().fg(Color::Black).bg(Color::Cyan))
                    .data(&data_to_show)
                    .max(1000);

                frame.render_widget(chart, cpu_chunks[1]);
            }
        }
    }
}

/// Render individual CPU cores with adaptive column layout
fn render_cpu_cores(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    let total_cores = app.render_cache.core_data.len();

    if total_cores == 0 || area.height == 0 {
        return;
    }

    let available_height = area.height as usize;
    let columns = if total_cores > available_height && area.width >= 60 {
        2
    } else {
        1
    };

    let cores_per_col = total_cores.div_ceil(columns);
    let cores_to_show = total_cores.min(available_height * columns);

    if columns == 2 {
        let col_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        for col in 0..2 {
            let start = col * cores_per_col;
            let end = (start + cores_per_col).min(cores_to_show);
            if start >= end {
                continue;
            }
            let rows_in_col = end - start;
            let col_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Length(1); rows_in_col])
                .split(col_chunks[col]);

            for (row, core_idx) in (start..end).enumerate() {
                render_single_core(frame, col_layout[row], app, core_idx);
            }
        }
    } else {
        let rows = cores_to_show.min(available_height);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(1); rows])
            .split(area);

        for i in 0..rows {
            render_single_core(frame, layout[i], app, i);
        }
    }
}

/// Render a single CPU core gauge using cached labels
fn render_single_core(frame: &mut Frame, area: Rect, app: &MonitorApp, core_idx: usize) {
    use ratatui::widgets::Gauge;

    if let Some(core) = app.render_cache.core_data.get(core_idx) {
        let label = if area.width < 35 {
            core.compact_label.as_str()
        } else {
            core.full_label.as_str()
        };

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(core.color))
            .label(label)
            .ratio((core.usage / 100.0).min(1.0) as f64);

        frame.render_widget(gauge, area);
    }
}

fn render_memory_gpu_section(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    if area.height < 3 {
        return;
    }

    let has_gpu = app.metrics.gpu.is_some();
    let constraints = if has_gpu {
        [Constraint::Percentage(50), Constraint::Percentage(50)]
    } else {
        [Constraint::Percentage(75), Constraint::Percentage(25)]
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    let border_style = if app.selected_tab == 1 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    // Memory
    let mem_block = Block::default()
        .title(" Memory ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let mem_inner = mem_block.inner(chunks[0]);
    frame.render_widget(mem_block, chunks[0]);

    let rc = &app.render_cache;

    if mem_inner.height < 3 {
        let summary = Paragraph::new(rc.mem_summary.as_str())
            .style(Style::default().fg(Color::White));
        frame.render_widget(summary, mem_inner);
    } else {
        let mem_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Length(2),
            ])
            .split(mem_inner);

        let ram_gauge = colored_gauge(rc.mem_display as f64, &rc.ram_text);
        frame.render_widget(ram_gauge, mem_layout[0]);

        let cache_text = Paragraph::new(rc.cache_text.as_str())
            .style(Style::default().fg(Color::Blue));
        frame.render_widget(cache_text, mem_layout[1]);

        let swap_gauge = colored_gauge(app.metrics.memory.swap_percent as f64, &rc.swap_text);
        frame.render_widget(swap_gauge, mem_layout[2]);
    }

    // GPU
    let gpu_block = Block::default()
        .title(" GPU ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let gpu_inner = gpu_block.inner(chunks[1]);
    frame.render_widget(gpu_block, chunks[1]);

    if app.metrics.gpu.is_some() {
        if gpu_inner.height < 3 {
            let summary = Paragraph::new(rc.gpu_summary.as_str())
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

            let name = Paragraph::new(rc.gpu_name.as_str())
                .style(Style::default().fg(Color::Cyan));
            frame.render_widget(name, gpu_layout[0]);

            let usage_gauge = colored_gauge(rc.gpu_display as f64, &rc.gpu_usage_text);
            frame.render_widget(usage_gauge, gpu_layout[1]);

            let details = Paragraph::new(rc.gpu_details.as_str())
                .style(Style::default().fg(Color::White));
            frame.render_widget(details, gpu_layout[2]);
        }
    } else {
        let no_gpu =
            Paragraph::new("No GPU detected").style(Style::default().fg(Color::DarkGray));
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
        return;
    }

    let rc = &app.render_cache;

    let net_rows: Vec<Row> = rc
        .net_rows
        .iter()
        .map(|net| {
            let error_style = if net.has_errors {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(net.interface.as_str()),
                Cell::from(net.rx_text.as_str()),
                Cell::from(net.tx_text.as_str()),
                Cell::from(net.errors_text.as_str()).style(error_style),
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

    // Disks
    let disk_block = Block::default()
        .title(" Storage Devices ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let disk_inner = disk_block.inner(chunks[1]);
    frame.render_widget(disk_block, chunks[1]);

    if rc.disk_entries.is_empty() {
        let no_data =
            Paragraph::new("No disks detected").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_data, disk_inner);
    } else {
        let available_height = disk_inner.height;
        let lines_per_disk = 3;
        let max_disks = ((available_height / lines_per_disk) as usize).min(3);

        let mut disk_lines = Vec::new();

        for (idx, disk) in rc.disk_entries.iter().take(max_disks).enumerate() {
            // Line 1: Type icon, mount point, SMART status, temperature
            let mut line1_spans = vec![
                Span::raw(disk.type_icon),
                Span::styled(
                    disk.mount_point.as_str(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(disk.smart_icon, Style::default().fg(disk.smart_color)),
            ];
            if disk.has_temp {
                line1_spans.push(Span::styled(
                    disk.temp_text.as_str(),
                    Style::default().fg(disk.temp_color),
                ));
            }
            let line1 = Line::from(line1_spans);

            // Line 2: Model + Interface
            let line2 = Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    disk.model_text.as_str(),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
                Span::styled(
                    disk.interface_text.as_str(),
                    Style::default().fg(Color::Cyan),
                ),
            ]);

            // Line 3: Usage bar (bar width depends on terminal — stays dynamic)
            let label_width =
                5 + disk.usage_pct_label.len() + disk.size_display.len() + disk.power_text.len();
            let bar_width = (disk_inner.width as usize)
                .saturating_sub(label_width)
                .clamp(5, 60);
            let filled = ((bar_width as f32 * disk.usage_pct / 100.0) as usize).min(bar_width);
            let empty = bar_width.saturating_sub(filled);

            let mut bar_spans = vec![Span::raw("  [")];
            // Use pre-allocated static bar characters instead of .repeat()
            let filled_bytes = filled * BLOCK_CHAR_BYTES;
            let empty_bytes = empty * BLOCK_CHAR_BYTES;
            if filled > 0 && filled_bytes <= BAR_FILLED.len() {
                bar_spans.push(Span::styled(
                    &BAR_FILLED[..filled_bytes],
                    Style::default().fg(disk.bar_color),
                ));
            }
            if empty > 0 && empty_bytes <= BAR_EMPTY.len() {
                bar_spans.push(Span::styled(
                    &BAR_EMPTY[..empty_bytes],
                    Style::default().fg(Color::DarkGray),
                ));
            }
            bar_spans.push(Span::raw("] "));
            bar_spans.push(Span::styled(
                disk.usage_pct_label.as_str(),
                Style::default()
                    .fg(disk.bar_color)
                    .add_modifier(Modifier::BOLD),
            ));
            bar_spans.push(Span::raw(disk.size_display.as_str()));

            if !disk.power_text.is_empty() {
                bar_spans.push(Span::styled(
                    disk.power_text.as_str(),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let line3 = Line::from(bar_spans);

            disk_lines.push(line1);
            disk_lines.push(line2);
            disk_lines.push(line3);

            if idx < max_disks - 1 && idx < rc.disk_entries.len() - 1 {
                disk_lines.push(Line::from(""));
            }
        }

        let disk_paragraph = Paragraph::new(disk_lines);
        frame.render_widget(disk_paragraph, disk_inner);
    }
}

fn render_processes_section(frame: &mut Frame, area: Rect, app: &mut MonitorApp) {
    let border_style = if app.selected_tab == 3 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let block = Block::default()
        .title(app.render_cache.process_title.as_str())
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let sort_by_mem = app.render_cache.process_sort_by_memory;
    let base_style = Style::default().add_modifier(Modifier::BOLD);
    let active_style = base_style.fg(Color::Cyan).add_modifier(Modifier::UNDERLINED);

    let header = Row::new(vec![
        Cell::from("PID").style(base_style),
        Cell::from("Name").style(base_style),
        Cell::from(if sort_by_mem { "CPU %" } else { "CPU % ▼" })
            .style(if sort_by_mem { base_style } else { active_style }),
        Cell::from(if sort_by_mem { "Memory ▼" } else { "Memory" })
            .style(if sort_by_mem { active_style } else { base_style }),
    ])
    .height(1);

    let rows: Vec<Row> = app
        .render_cache
        .process_rows
        .iter()
        .map(|p| {
            Row::new(vec![
                Cell::from(p.pid.as_str()),
                Cell::from(p.name.as_str()),
                Cell::from(p.cpu.as_str()),
                Cell::from(p.mem.as_str()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Percentage(45),
            Constraint::Length(10),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, inner, &mut app.process_table_state);
}

fn render_temperatures_section(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    if area.height == 0 {
        return;
    }

    let rc = &app.render_cache;

    if rc.temp_entries.is_empty() {
        let para = Paragraph::new(" Temperatures: No sensors detected")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(para, area);
        return;
    }

    let mut spans = vec![Span::styled(" Temps: ", Style::default().fg(Color::White))];

    for (i, t) in rc.temp_entries.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        }
        spans.push(Span::styled(t.text.as_str(), Style::default().fg(t.color)));
    }

    let para = Paragraph::new(Line::from(spans));
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

    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(paragraph, popup_area);
}

/// Shown when the terminal is too small for the dashboard layout.
fn render_too_small_message(frame: &mut Frame, area: Rect) {
    let msg = format!(
        "Terminal too small: {}x{}\nMinimum: {}x{}\n\nResize to continue.",
        area.width, area.height, MIN_WIDTH, MIN_HEIGHT
    );

    let paragraph = Paragraph::new(msg)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);

    // Vertically center the message
    if area.height >= 4 {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(4),
                Constraint::Fill(1),
            ])
            .split(area);
        frame.render_widget(paragraph, layout[1]);
    } else {
        frame.render_widget(paragraph, area);
    }
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
