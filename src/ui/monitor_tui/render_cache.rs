use humansize::{format_size as human_format_size, DECIMAL};
use ratatui::style::Color;

/// Format bytes into human-readable size string.
pub fn fmt_size(bytes: u64) -> String {
    human_format_size(bytes, DECIMAL)
}

/// Format duration in seconds to human-readable format.
pub fn fmt_duration(secs: u64) -> String {
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

// Pre-allocated bar characters (max 60 blocks each).
// Avoids String::repeat() heap allocation in the render loop.
// Each block character is 3 bytes in UTF-8.
pub const BAR_FILLED: &str = concat!(
    "██████████",
    "██████████",
    "██████████",
    "██████████",
    "██████████",
    "██████████",
);
pub const BAR_EMPTY: &str = concat!(
    "░░░░░░░░░░",
    "░░░░░░░░░░",
    "░░░░░░░░░░",
    "░░░░░░░░░░",
    "░░░░░░░░░░",
    "░░░░░░░░░░",
);

/// Byte size of "█" and "░" in UTF-8 (both are 3 bytes).
pub const BLOCK_CHAR_BYTES: usize = "█".len();

/// Cached per-core CPU render data.
pub struct CachedCore {
    pub compact_label: String,
    pub full_label: String,
    pub usage: f32,
    pub color: Color,
}

/// Cached network interface render data.
pub struct CachedNetRow {
    pub interface: String,
    pub rx_text: String,
    pub tx_text: String,
    pub errors_text: String,
    pub has_errors: bool,
}

/// Cached disk render data.
pub struct CachedDisk {
    pub type_icon: &'static str,
    pub mount_point: String,
    pub smart_icon: &'static str,
    pub smart_color: Color,
    pub temp_text: String,
    pub temp_color: Color,
    pub has_temp: bool,
    pub model_text: String,
    pub interface_text: String,
    pub usage_pct: f32,
    pub bar_color: Color,
    pub usage_pct_label: String,
    pub size_display: String,
    pub power_text: String,
}

/// Cached process row render data.
pub struct CachedProcess {
    pub pid: String,
    pub name: String,
    pub cpu: String,
    pub mem: String,
}

/// Cached temperature sensor render data.
pub struct CachedTemp {
    pub text: String,
    pub color: Color,
}

/// Pre-computed formatted strings to avoid heap allocations during rendering.
///
/// Rebuilt only when metrics change (~1/second) or on relevant UI state changes.
/// The render loop borrows these strings without allocating, reducing per-frame
/// heap allocations from ~120-180 to near zero.
pub struct RenderCache {
    pub global_title: String,
    pub power_color: Color,

    pub cpu_title: String,
    pub core_data: Vec<CachedCore>,

    pub mem_display: f32,
    pub ram_text: String,
    pub cache_text: String,
    pub swap_text: String,
    pub mem_summary: String,

    pub gpu_display: f32,
    pub gpu_name: String,
    pub gpu_usage_text: String,
    pub gpu_details: String,
    pub gpu_summary: String,

    pub net_rows: Vec<CachedNetRow>,

    pub disk_entries: Vec<CachedDisk>,

    pub process_title: String,
    pub process_sort_by_memory: bool,
    pub process_rows: Vec<CachedProcess>,

    pub temp_entries: Vec<CachedTemp>,
}

impl RenderCache {
    pub fn new() -> Self {
        Self {
            global_title: String::new(),
            power_color: Color::White,
            cpu_title: String::new(),
            core_data: Vec::new(),
            mem_display: 0.0,
            ram_text: String::new(),
            cache_text: String::new(),
            swap_text: String::new(),
            mem_summary: String::new(),
            gpu_display: 0.0,
            gpu_name: String::new(),
            gpu_usage_text: String::new(),
            gpu_details: String::new(),
            gpu_summary: String::new(),
            net_rows: Vec::new(),
            disk_entries: Vec::new(),
            process_title: String::new(),
            process_sort_by_memory: false,
            process_rows: Vec::new(),
            temp_entries: Vec::new(),
        }
    }
}
