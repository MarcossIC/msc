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

