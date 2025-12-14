use ratatui::{prelude::*, widgets::Gauge};

/// Create a gauge with color based on value thresholds
pub fn colored_gauge<'a>(value: f64, label: &'a str) -> Gauge<'a> {
    let color = match value {
        v if v < 50.0 => Color::Cyan,        // Mejor contraste que Green
        v if v < 75.0 => Color::LightYellow, // Mejor contraste que Yellow
        v if v < 90.0 => Color::LightRed,
        _ => Color::Red,
    };

    Gauge::default()
        .gauge_style(Style::default().fg(color).bg(Color::Black)) // Negro en vez de DarkGray para mejor contraste
        .ratio(value / 100.0)
        .label(label)
}

/// Get color for temperature value
pub fn temp_color(temp: f32) -> Color {
    match temp {
        t if t < 50.0 => Color::Cyan,        // Mejor contraste que Green
        t if t < 70.0 => Color::LightYellow, // Mejor contraste que Yellow
        t if t < 85.0 => Color::LightRed,
        _ => Color::Red,
    }
}

/// Format bytes per second for network display
#[allow(dead_code)]
pub fn format_speed(bytes_per_sec: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes_per_sec >= GB {
        format!("{:.2} GB/s", bytes_per_sec as f64 / GB as f64)
    } else if bytes_per_sec >= MB {
        format!("{:.2} MB/s", bytes_per_sec as f64 / MB as f64)
    } else if bytes_per_sec >= KB {
        format!("{:.2} KB/s", bytes_per_sec as f64 / KB as f64)
    } else {
        format!("{} B/s", bytes_per_sec)
    }
}
