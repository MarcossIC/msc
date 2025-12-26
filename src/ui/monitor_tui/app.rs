use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::core::system_monitor::{
    evaluate_alerts, Alert, AlertConfig, MetricsHistory, MetricsRuntime, SystemMetrics,
};

use super::event_handler::MonitorEvent;
use super::render::render_ui;

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
    // Smoothed values for fluid animations
    pub smoothed_cpu_usage: f32,
    pub smoothed_memory_usage: f32,
    pub smoothed_gpu_usage: f32,
    pub smoothed_per_core: Vec<f32>,
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
            smoothed_cpu_usage: 0.0,
            smoothed_memory_usage: 0.0,
            smoothed_gpu_usage: 0.0,
            smoothed_per_core: Vec::new(),
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

            return true;
        }
        false
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
            }
            MonitorEvent::ToggleProcessTree => {
                self.show_process_tree = !self.show_process_tree;
                self.selected_process_index = 0; // Reset selection
            }
            MonitorEvent::ProcessUp => {
                if self.selected_process_index > 0 {
                    self.selected_process_index -= 1;
                }
            }
            MonitorEvent::ProcessDown => {
                let max_index = self.metrics.top_processes.len().saturating_sub(1);
                if self.selected_process_index < max_index {
                    self.selected_process_index += 1;
                }
            }
            MonitorEvent::None => {}
        }
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

    // Target 60 FPS
    let frame_duration = Duration::from_millis(16);
    let mut last_frame = Instant::now();

    // Main loop
    loop {
        // Non-blocking metrics update
        app.try_update_metrics();

        // Draw UI
        terminal.draw(|frame| render_ui(frame, &app))?;

        // Handle events with minimal timeout (just to be responsive)
        // We poll for a very short time to keep the loop tight but responsive
        if event::poll(Duration::from_millis(1)).context("Event poll failed")? {
            if let Event::Key(key) = event::read().context("Event read failed")? {
                if key.kind == KeyEventKind::Press {
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

        // Check if should quit
        if app.should_quit {
            break;
        }

        // Maintain 60 FPS
        let elapsed = last_frame.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
        last_frame = Instant::now();
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
