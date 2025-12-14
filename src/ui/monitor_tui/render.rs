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

    // Create main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(8), // CPU
            Constraint::Length(6), // Memory + GPU row
            Constraint::Length(5), // Network + Disk
            Constraint::Min(8),    // Processes
            Constraint::Length(2), // Temperatures
            Constraint::Length(1), // Footer
        ])
        .split(area);

    render_header(frame, chunks[0], app);
    render_cpu_section(frame, chunks[1], app);
    render_memory_gpu_section(frame, chunks[2], app);
    render_network_disk_section(frame, chunks[3], app);
    render_processes_section(frame, chunks[4], app);
    render_temperatures_section(frame, chunks[5], app);
    render_footer(frame, chunks[6]);

    // Render help overlay if active
    if app.show_help {
        render_help_overlay(frame, area);
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    let title = format!(
        " MSC System Monitor │ {} │ Refresh: {}ms ",
        chrono::Local::now().format("%H:%M:%S"),
        app.interval_ms
    );
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);
}

fn render_cpu_section(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    let cpu = &app.metrics.cpu;

    let block = Block::default()
        .title(format!(
            " CPU: {} ({} cores) │ Load: {:.2} {:.2} {:.2} ",
            cpu.brand, cpu.core_count, cpu.load_average.0, cpu.load_average.1, cpu.load_average.2,
        ))
        .borders(Borders::ALL);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split for gauge and sparkline
    let cpu_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(inner);

    // CPU Usage gauge
    let gauge = colored_gauge(cpu.global_usage as f64, "Total");
    frame.render_widget(gauge, cpu_chunks[0]);

    // CPU History sparkline
    let sparkline = Sparkline::default()
        .block(Block::default().title("History").borders(Borders::ALL))
        .data(app.history.cpu_as_u64())
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(sparkline, cpu_chunks[1]);
}

fn render_memory_gpu_section(frame: &mut Frame, area: Rect, app: &MonitorApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Memory
    let mem = &app.metrics.memory;
    let mem_block = Block::default().title(" Memory ").borders(Borders::ALL);
    let mem_inner = mem_block.inner(chunks[0]);
    frame.render_widget(mem_block, chunks[0]);

    let mem_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(2)])
        .split(mem_inner);

    let ram_text = format!(
        "RAM:  {} / {} ({:.1}%)",
        format_size(mem.used_bytes),
        format_size(mem.total_bytes),
        mem.usage_percent
    );
    let ram_gauge = colored_gauge(mem.usage_percent as f64, &ram_text);
    frame.render_widget(ram_gauge, mem_layout[0]);

    let swap_text = format!(
        "SWAP: {} / {} ({:.1}%)",
        format_size(mem.swap_used_bytes),
        format_size(mem.swap_total_bytes),
        mem.swap_percent
    );
    let swap_gauge = colored_gauge(mem.swap_percent as f64, &swap_text);
    frame.render_widget(swap_gauge, mem_layout[1]);

    // GPU
    let gpu_block = Block::default().title(" GPU ").borders(Borders::ALL);
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

    // Network
    let net_block = Block::default().title(" Network ").borders(Borders::ALL);
    let net_inner = net_block.inner(chunks[0]);
    frame.render_widget(net_block, chunks[0]);

    let net_rows: Vec<Row> = app
        .metrics
        .network
        .iter()
        .take(3)
        .map(|net| {
            Row::new(vec![
                Cell::from(net.interface.clone()),
                Cell::from(format!("↓ {}/s", format_size(net.rx_bytes_per_sec))),
                Cell::from(format!("↑ {}/s", format_size(net.tx_bytes_per_sec))),
            ])
        })
        .collect();

    let net_table = Table::new(
        net_rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ],
    );
    frame.render_widget(net_table, net_inner);

    // Disks
    let disk_block = Block::default().title(" Disks ").borders(Borders::ALL);
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
    let block = Block::default()
        .title(" Top Processes (press 's' to toggle sort) ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let header = Row::new(vec![
        Cell::from("PID").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("CPU %").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Memory").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .height(1);

    let rows: Vec<Row> = app
        .metrics
        .top_processes
        .iter()
        .map(|proc| {
            Row::new(vec![
                Cell::from(proc.pid.to_string()),
                Cell::from(proc.name.clone()),
                Cell::from(format!("{:.1}%", proc.cpu_usage_percent)),
                Cell::from(format_size(proc.memory_bytes)),
                Cell::from(proc.status.clone()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Percentage(40),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

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
