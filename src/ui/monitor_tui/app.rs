use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, widgets::TableState, Terminal};

use ratatui::style::Color;

use crate::core::system_monitor::{
    build_process_tree, evaluate_alerts, flatten_tree, format_tree_indent, Alert, AlertConfig,
    DiskType, FlattenedProcess, MetricsHistory, MetricsRuntime, PowerSource, SmartStatus,
    SystemMetrics,
};

use super::event_handler::MonitorEvent;
use super::render::render_ui;
use super::render_cache::{
    fmt_duration, fmt_size, CachedCore, CachedDisk, CachedNetRow, CachedProcess, CachedTemp,
    RenderCache,
};
use super::widgets::temp_color;

/// Monitor application state
pub struct MonitorApp {
    pub metrics: Arc<SystemMetrics>,
    pub history: MetricsHistory,
    pub runtime: MetricsRuntime,
    pub should_quit: bool,
    pub show_help: bool,
    pub selected_tab: usize,
    pub process_sort_by_memory: bool,
    pub interval_ms: u64,
    pub show_process_tree: bool,
    pub selected_process_index: usize,
    pub alerts: Vec<Alert>,
    pub alert_config: AlertConfig,
    pub process_table_state: TableState,
    // Smoothed values for fluid animations
    pub smoothed_cpu_usage: f32,
    pub smoothed_memory_usage: f32,
    pub smoothed_gpu_usage: f32,
    pub smoothed_per_core: Vec<f32>,
    // Cached process tree (rebuilt only on metrics update)
    pub cached_flattened_processes: Vec<FlattenedProcess>,
    // Pre-computed render strings — rebuilt on metrics update, borrowed during render
    pub render_cache: RenderCache,
    // Dirty flag: true when UI needs to re-render
    pub needs_redraw: bool,
}

impl MonitorApp {
    pub fn new(config: MonitorAppConfig) -> Result<Self> {
        let runtime = MetricsRuntime::new()?;

        Ok(Self {
            metrics: Arc::new(SystemMetrics::default()),
            history: MetricsHistory::new(),
            runtime,
            should_quit: false,
            show_help: false,
            selected_tab: 0,
            process_sort_by_memory: false,
            interval_ms: config.interval_ms,
            show_process_tree: true, // Default to tree view
            selected_process_index: 0,
            alerts: Vec::new(),
            alert_config: AlertConfig::default(),
            process_table_state: TableState::default().with_selected(Some(0)),
            smoothed_cpu_usage: 0.0,
            smoothed_memory_usage: 0.0,
            smoothed_gpu_usage: 0.0,
            smoothed_per_core: Vec::new(),
            cached_flattened_processes: Vec::new(),
            render_cache: RenderCache::new(),
            needs_redraw: true,
        })
    }

    /// Non-blocking update from async runtime
    pub fn try_update_metrics(&mut self) -> bool {
        if self.runtime.snapshot_rx.has_changed().unwrap_or(false) {
            self.metrics = self.runtime.snapshot_rx.borrow().clone();

            // Apply smoothing to metrics (Exponential Moving Average)
            // Alpha = 0.3 means 30% new value, 70% old value (smooth transitions)
            const ALPHA: f32 = 0.3;

            self.smoothed_cpu_usage = self.smooth_value(
                self.smoothed_cpu_usage,
                self.metrics.cpu.global_usage,
                ALPHA,
            );

            self.smoothed_memory_usage = self.smooth_value(
                self.smoothed_memory_usage,
                self.metrics.memory.usage_percent,
                ALPHA,
            );

            if let Some(ref gpu) = self.metrics.gpu {
                self.smoothed_gpu_usage = self.smooth_value(
                    self.smoothed_gpu_usage,
                    gpu.utilization_percent as f32,
                    ALPHA,
                );
            }

            // Smooth per-core CPU usage
            if self.smoothed_per_core.len() != self.metrics.cpu.per_core_usage.len() {
                // Initialize if size changed
                self.smoothed_per_core = self.metrics.cpu.per_core_usage.clone();
            } else {
                for (i, &current) in self.metrics.cpu.per_core_usage.iter().enumerate() {
                    self.smoothed_per_core[i] =
                        self.smooth_value(self.smoothed_per_core[i], current, ALPHA);
                }
            }

            // Update history for sparklines (use smoothed values)
            self.history.push_cpu(self.smoothed_cpu_usage);
            self.history.push_memory(self.smoothed_memory_usage);

            if self.metrics.gpu.is_some() {
                self.history.push_gpu(self.smoothed_gpu_usage as u32);
            }

            if let Some(net) = self.metrics.network.first() {
                self.history
                    .push_network(net.rx_bytes_per_sec, net.tx_bytes_per_sec);
            }

            // Evaluate alerts
            self.alerts = evaluate_alerts(&self.metrics, &self.alert_config);

            // Rebuild process tree cache
            self.rebuild_process_cache();

            // Clamp selection index — process count can change between updates
            let max_idx = self.cached_process_count().saturating_sub(1);
            if self.selected_process_index > max_idx {
                self.selected_process_index = max_idx;
            }

            self.rebuild_render_cache();
            self.needs_redraw = true;
            return true;
        }
        false
    }

    /// Rebuild cached process tree/list from current metrics
    fn rebuild_process_cache(&mut self) {
        if self.show_process_tree {
            let tree = build_process_tree(&self.metrics.top_processes);
            self.cached_flattened_processes = flatten_tree(&tree);
        } else {
            // En modo lista, creamos FlattenedProcess simples sin jerarquía
            self.cached_flattened_processes = self
                .metrics
                .top_processes
                .iter()
                .map(|p| FlattenedProcess {
                    process: p.clone(),
                    depth: 0,
                    is_last: false,
                    parent_chain: Vec::new(),
                })
                .collect();
        }
    }

    /// Rebuild all pre-computed render strings from current metrics and smoothed values.
    /// Called on metrics update to avoid per-frame heap allocations in the render loop.
    fn rebuild_render_cache(&mut self) {
        // Phase 1: Everything except processes (scoped to release borrows)
        {
            let rc = &mut self.render_cache;
            let metrics = &self.metrics;
            let smoothed_cpu = self.smoothed_cpu_usage;
            let smoothed_mem = self.smoothed_memory_usage;
            let smoothed_gpu = self.smoothed_gpu_usage;
            let interval = self.interval_ms;
            let per_core = &self.smoothed_per_core;

            // === Global Dashboard ===
            let global = &metrics.global;
            let uptime_str = fmt_duration(global.uptime_secs);
            let load = metrics.cpu.load_average;
            let power_str: String = match global.power_source {
                PowerSource::AC => "⚡ AC".into(),
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
                        "🔋 Battery".into()
                    }
                }
                PowerSource::Unknown => "? Unknown".into(),
            };
            rc.power_color = if global.power_source == PowerSource::Battery {
                match global.battery_percent {
                    Some(pct) if pct < 20.0 => Color::Red,
                    Some(pct) if pct < 50.0 => Color::Yellow,
                    Some(_) => Color::Green,
                    None => Color::White,
                }
            } else {
                Color::Cyan
            };
            rc.global_title = format!(
                " {} │ Uptime: {} │ Load: {:.2} {:.2} {:.2} │ {} │ Refresh: {}ms ",
                global.hostname, uptime_str, load.0, load.1, load.2, power_str, interval
            );

            // === CPU ===
            let cpu = &metrics.cpu;
            let avg_freq = if !cpu.frequencies_mhz.is_empty() {
                cpu.frequencies_mhz.iter().sum::<u64>() / cpu.frequencies_mhz.len() as u64
            } else {
                0
            };
            let cpu_display = if smoothed_cpu > 0.0 {
                smoothed_cpu
            } else {
                cpu.global_usage
            };
            rc.cpu_title = format!(
                " CPU: {} ({} cores) │ Avg: {:.1}% @ {} MHz │ Load: {:.2} {:.2} {:.2} ",
                cpu.brand, cpu.core_count, cpu_display, avg_freq, load.0, load.1, load.2,
            );

            rc.core_data.clear();
            for i in 0..cpu.per_core_usage.len() {
                let usage = per_core
                    .get(i)
                    .copied()
                    .unwrap_or_else(|| cpu.per_core_usage.get(i).copied().unwrap_or(0.0));
                let freq = cpu.frequencies_mhz.get(i).copied().unwrap_or(0);
                let color = if usage > 90.0 {
                    Color::Red
                } else if usage > 75.0 {
                    Color::Yellow
                } else if usage > 50.0 {
                    Color::Cyan
                } else {
                    Color::Green
                };
                let hot = if usage > 90.0 { " !" } else { "" };
                rc.core_data.push(CachedCore {
                    compact_label: format!("C{:02} {:>5.1}%{}", i, usage, hot),
                    full_label: format!("C{:02} [{:>5.1}%] @ {:>4} MHz{}", i, usage, freq, hot),
                    usage,
                    color,
                });
            }

            // === Memory ===
            let mem = &metrics.memory;
            rc.mem_display = if smoothed_mem > 0.0 {
                smoothed_mem
            } else {
                mem.usage_percent
            };
            rc.ram_text = format!(
                "Used:  {} / {} ({:.1}%)",
                fmt_size(mem.used_bytes),
                fmt_size(mem.total_bytes),
                rc.mem_display
            );
            let cache_pct = if mem.total_bytes > 0 {
                (mem.cache_buffers_bytes as f32 / mem.total_bytes as f32) * 100.0
            } else {
                0.0
            };
            rc.cache_text = format!(
                "Cache: {} ({:.1}%)",
                fmt_size(mem.cache_buffers_bytes),
                cache_pct
            );
            rc.swap_text = format!(
                "Swap:  {} / {} ({:.1}%)",
                fmt_size(mem.swap_used_bytes),
                fmt_size(mem.swap_total_bytes),
                mem.swap_percent
            );
            rc.mem_summary = format!("RAM: {:.1}%", rc.mem_display);

            // === GPU ===
            if let Some(ref gpu) = metrics.gpu {
                rc.gpu_display = if smoothed_gpu > 0.0 {
                    smoothed_gpu
                } else {
                    gpu.utilization_percent as f32
                };
                rc.gpu_name.clear();
                rc.gpu_name.push_str(&gpu.name);
                rc.gpu_usage_text = format!(
                    "Use: {:.0}% │ VRAM: {} / {}",
                    rc.gpu_display,
                    fmt_size(gpu.memory_used_bytes),
                    fmt_size(gpu.memory_total_bytes)
                );
                rc.gpu_details = format!(
                    "Temp: {}°C │ Fan: {}% │ Power: {}W/{}W",
                    gpu.temperature_celsius
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "N/A".into()),
                    gpu.fan_speed_percent
                        .map(|f| f.to_string())
                        .unwrap_or_else(|| "N/A".into()),
                    gpu.power_draw_watts
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "N/A".into()),
                    gpu.power_limit_watts
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "N/A".into()),
                );
                rc.gpu_summary = format!("GPU: {:.0}%", rc.gpu_display);
            } else {
                rc.gpu_display = 0.0;
                rc.gpu_name.clear();
                rc.gpu_usage_text.clear();
                rc.gpu_details.clear();
                rc.gpu_summary.clear();
            }

            // === Network ===
            rc.net_rows.clear();
            for net in metrics.network.iter().take(3) {
                let has_errors = net.rx_errors > 0 || net.tx_errors > 0;
                rc.net_rows.push(CachedNetRow {
                    interface: net.interface.clone(),
                    rx_text: format!("↓ {}/s", fmt_size(net.rx_bytes_per_sec)),
                    tx_text: format!("↑ {}/s", fmt_size(net.tx_bytes_per_sec)),
                    errors_text: if has_errors {
                        format!("⚠ {}", net.rx_errors + net.tx_errors)
                    } else {
                        "✓".into()
                    },
                    has_errors,
                });
            }

            // === Disks ===
            rc.disk_entries.clear();
            for disk in metrics.disks.iter().take(3) {
                let type_icon = match disk.disk_type.as_ref() {
                    Some(DiskType::NVMe) => "⚡ ",
                    Some(DiskType::SSD) => "💿 ",
                    Some(DiskType::HDD) => "💾 ",
                    _ => "📀 ",
                };
                let (smart_icon, smart_color) = match disk.smart_status.as_ref() {
                    Some(SmartStatus::Healthy) => ("✓", Color::Green),
                    Some(SmartStatus::Warning) => ("⚠", Color::Yellow),
                    Some(SmartStatus::Critical) => ("✗", Color::Red),
                    _ => ("?", Color::DarkGray),
                };
                let (has_temp, temp_text, temp_color_val) =
                    if let Some(temp) = disk.temperature_celsius {
                        let tc = if temp > 60 {
                            Color::Red
                        } else if temp > 45 {
                            Color::Yellow
                        } else {
                            Color::Green
                        };
                        (true, format!(" {}°C", temp), tc)
                    } else {
                        (false, String::new(), Color::White)
                    };
                let model_text = match (&disk.manufacturer, &disk.model) {
                    (Some(mfr), Some(model)) => format!("{} {}", mfr, model),
                    (Some(mfr), None) => mfr.clone(),
                    (None, Some(model)) => model.clone(),
                    (None, None) => disk.fs_type.clone(),
                };
                let interface_text = disk
                    .interface_speed
                    .as_ref()
                    .map(|s| format!(" • {}", s))
                    .unwrap_or_default();
                let used = disk.total_bytes - disk.available_bytes;
                let usage_pct = disk.usage_percent;
                let used_fmt = fmt_size(used);
                let total_fmt = fmt_size(disk.total_bytes);
                let bar_color = if usage_pct > 90.0 {
                    Color::Red
                } else if usage_pct > 75.0 {
                    Color::Yellow
                } else {
                    Color::Green
                };
                rc.disk_entries.push(CachedDisk {
                    type_icon,
                    mount_point: disk.mount_point.clone(),
                    smart_icon,
                    smart_color,
                    has_temp,
                    temp_text,
                    temp_color: temp_color_val,
                    model_text,
                    interface_text,
                    usage_pct,
                    bar_color,
                    usage_pct_label: format!("{:.1}% ", usage_pct),
                    size_display: format!("{}/{}", &used_fmt, &total_fmt),
                    power_text: disk
                        .power_on_hours
                        .map(|h| format!(" • {}d", h / 24))
                        .unwrap_or_default(),
                });
            }

            // === Temperatures ===
            rc.temp_entries.clear();
            for t in metrics.temperatures.iter().take(6) {
                rc.temp_entries.push(CachedTemp {
                    text: format!("{}: {:.0}°C", t.label, t.current_celsius),
                    color: temp_color(t.current_celsius),
                });
            }
        }

        // Phase 2: Processes (needs separate &mut self borrow)
        self.rebuild_process_render_cache();
    }

    /// Rebuild only the process-related render cache.
    fn rebuild_process_render_cache(&mut self) {
        let rc = &mut self.render_cache;

        let mode_str = if self.show_process_tree {
            "Tree"
        } else {
            "List"
        };
        let sort_str = if self.process_sort_by_memory {
            "▼Mem"
        } else {
            "▼CPU"
        };
        let total = self.cached_flattened_processes.len();
        let pos = if total > 0 {
            format!(" {}/{} ", self.selected_process_index + 1, total)
        } else {
            String::new()
        };
        rc.process_title = format!(
            " Processes ({} │ {}) [t:toggle s:sort ↑↓:nav]{} ",
            mode_str, sort_str, pos
        );
        rc.process_sort_by_memory = self.process_sort_by_memory;

        rc.process_rows.clear();
        for flat_proc in &self.cached_flattened_processes {
            let proc_m = &flat_proc.process;
            let name = if self.show_process_tree {
                let indent = format_tree_indent(flat_proc);
                format!("{}{}", indent, proc_m.name)
            } else {
                proc_m.name.clone()
            };
            rc.process_rows.push(CachedProcess {
                pid: proc_m.pid.to_string(),
                name,
                cpu: format!("{:.1}%", proc_m.cpu_usage_percent),
                mem: fmt_size(proc_m.memory_bytes),
            });
        }
    }

    /// Update just the process title string (for navigation/sort events).
    fn update_process_title(&mut self) {
        let mode_str = if self.show_process_tree {
            "Tree"
        } else {
            "List"
        };
        let sort_str = if self.process_sort_by_memory {
            "▼Mem"
        } else {
            "▼CPU"
        };
        let total = self.cached_flattened_processes.len();
        let pos = if total > 0 {
            format!(" {}/{} ", self.selected_process_index + 1, total)
        } else {
            String::new()
        };
        self.render_cache.process_title = format!(
            " Processes ({} │ {}) [t:toggle s:sort ↑↓:nav]{} ",
            mode_str, sort_str, pos
        );
        self.render_cache.process_sort_by_memory = self.process_sort_by_memory;
    }

    /// Returns cached process count (O(1), no rebuild)
    pub fn cached_process_count(&self) -> usize {
        self.cached_flattened_processes.len()
    }

    /// Smooth a value using Exponential Moving Average
    /// Alpha controls smoothing: 0.0 = no change, 1.0 = instant change
    /// Lower alpha = smoother but slower response
    fn smooth_value(&self, old_value: f32, new_value: f32, alpha: f32) -> f32 {
        if old_value == 0.0 {
            // First value, no smoothing
            new_value
        } else {
            // EMA: new_smoothed = alpha * new + (1 - alpha) * old
            alpha * new_value + (1.0 - alpha) * old_value
        }
    }

    /// Handle keyboard/mouse events
    pub fn handle_event(&mut self, event: MonitorEvent) {
        if event == MonitorEvent::None {
            return;
        }

        match event {
            MonitorEvent::Quit => self.should_quit = true,
            MonitorEvent::ToggleHelp => self.show_help = !self.show_help,
            MonitorEvent::NextTab => self.selected_tab = (self.selected_tab + 1) % 4,
            MonitorEvent::PrevTab => {
                self.selected_tab = if self.selected_tab == 0 {
                    3
                } else {
                    self.selected_tab - 1
                };
            }
            MonitorEvent::ToggleProcessSort => {
                self.process_sort_by_memory = !self.process_sort_by_memory;
                self.update_process_title();
            }
            MonitorEvent::ToggleProcessTree => {
                // Remember the selected process PID before rebuilding
                let selected_pid = self
                    .cached_flattened_processes
                    .get(self.selected_process_index)
                    .map(|fp| fp.process.pid);

                self.show_process_tree = !self.show_process_tree;
                self.rebuild_process_cache();

                // Restore selection by PID, fall back to 0 if not found
                self.selected_process_index = selected_pid
                    .and_then(|pid| {
                        self.cached_flattened_processes
                            .iter()
                            .position(|fp| fp.process.pid == pid)
                    })
                    .unwrap_or(0);

                self.rebuild_process_render_cache();
            }
            MonitorEvent::ProcessUp => {
                if self.selected_process_index > 0 {
                    self.selected_process_index -= 1;
                    self.update_process_title();
                }
            }
            MonitorEvent::ProcessDown => {
                let max_index = self.cached_process_count().saturating_sub(1);
                if self.selected_process_index < max_index {
                    self.selected_process_index += 1;
                    self.update_process_title();
                }
            }
            MonitorEvent::None => unreachable!(),
        }

        self.needs_redraw = true;
        // Keep TableState in sync with selected index for scroll viewport
        self.process_table_state
            .select(Some(self.selected_process_index));
    }
}

/// Configuration for the monitor app
#[derive(Debug, Clone)]
pub struct MonitorAppConfig {
    pub interval_ms: u64,
    pub show_cpu: bool,
    pub show_memory: bool,
    pub show_gpu: bool,
    pub show_disks: bool,
    pub show_network: bool,
    pub show_temperatures: bool,
    pub show_processes: bool,
    pub top_processes: usize,
}

impl Default for MonitorAppConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            show_cpu: true,
            show_memory: true,
            show_gpu: true,
            show_disks: true,
            show_network: true,
            show_temperatures: true,
            show_processes: true,
            top_processes: 10,
        }
    }
}

/// Run the monitor TUI application
pub fn run_monitor_app(config: MonitorAppConfig) -> Result<()> {
    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Create app
    let mut app = MonitorApp::new(config)?;

    // Main loop — render only when state changes (metrics update or user input)
    loop {
        // Non-blocking metrics update
        app.try_update_metrics();

        // Process ALL pending events (no input lag on held keys)
        while event::poll(Duration::from_millis(0)).context("Event poll failed")? {
            if let Event::Key(key) = event::read().context("Event read failed")? {
                if key.kind == KeyEventKind::Press {
                    // When help overlay is open, ANY key dismisses it
                    if app.show_help {
                        app.handle_event(MonitorEvent::ToggleHelp);
                    } else {
                        let monitor_event = match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => MonitorEvent::Quit,
                            KeyCode::Char('?') | KeyCode::Char('h') => MonitorEvent::ToggleHelp,
                            KeyCode::Tab => MonitorEvent::NextTab,
                            KeyCode::BackTab => MonitorEvent::PrevTab,
                            KeyCode::Char('s') => MonitorEvent::ToggleProcessSort,
                            KeyCode::Char('t') => MonitorEvent::ToggleProcessTree,
                            KeyCode::Up | KeyCode::Char('k') => MonitorEvent::ProcessUp,
                            KeyCode::Down | KeyCode::Char('j') => MonitorEvent::ProcessDown,
                            _ => MonitorEvent::None,
                        };
                        app.handle_event(monitor_event);
                    }
                }
            }
        }

        // Check if should quit
        if app.should_quit {
            break;
        }

        // Only render when something changed
        if app.needs_redraw {
            terminal.draw(|frame| render_ui(frame, &mut app))?;
            app.needs_redraw = false;
        }

        // Sleep until next potential event — no busy loop
        // 50ms gives responsive input while saving CPU (vs 16ms at 60fps)
        std::thread::sleep(Duration::from_millis(50));
    }

    // Cleanup runtime
    app.runtime.shutdown();

    // Restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    Ok(())
}
