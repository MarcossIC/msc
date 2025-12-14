use std::io;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::core::system_monitor::{
    evaluate_alerts, Alert, AlertConfig, CollectorConfig, MetricsCollector, MetricsHistory,
    SystemMetrics,
};

use super::event_handler::MonitorEvent;
use super::render::render_ui;

/// Monitor application state
pub struct MonitorApp {
    pub metrics: SystemMetrics,
    pub history: MetricsHistory,
    pub collector: MetricsCollector,
    pub should_quit: bool,
    pub show_help: bool,
    pub selected_tab: usize,
    pub process_sort_by_memory: bool,
    pub interval_ms: u64,
    pub show_process_tree: bool,
    pub selected_process_index: usize,
    pub alerts: Vec<Alert>,
    pub alert_config: AlertConfig,
}

impl MonitorApp {
    pub fn new(config: MonitorAppConfig) -> Self {
        let collector_config = CollectorConfig {
            collect_cpu: config.show_cpu,
            collect_memory: config.show_memory,
            collect_gpu: config.show_gpu,
            collect_disks: config.show_disks,
            collect_network: config.show_network,
            collect_temperatures: config.show_temperatures,
            collect_processes: config.show_processes,
            top_processes_count: config.top_processes,
        };

        Self {
            metrics: SystemMetrics::default(),
            history: MetricsHistory::new(),
            collector: MetricsCollector::with_config(collector_config),
            should_quit: false,
            show_help: false,
            selected_tab: 0,
            process_sort_by_memory: false,
            interval_ms: config.interval_ms,
            show_process_tree: true, // Default to tree view
            selected_process_index: 0,
            alerts: Vec::new(),
            alert_config: AlertConfig::default(),
        }
    }

    /// Update metrics from collector
    pub fn update_metrics(&mut self) -> Result<()> {
        self.metrics = self
            .collector
            .collect()
            .context("Failed to collect metrics")?;

        // Update history for sparklines
        self.history.push_cpu(self.metrics.cpu.global_usage);
        self.history.push_memory(self.metrics.memory.usage_percent);

        if let Some(ref gpu) = self.metrics.gpu {
            self.history.push_gpu(gpu.utilization_percent);
        }

        if let Some(net) = self.metrics.network.first() {
            self.history
                .push_network(net.rx_bytes_per_sec, net.tx_bytes_per_sec);
        }

        // Evaluate alerts
        self.alerts = evaluate_alerts(&self.metrics, &self.alert_config);

        Ok(())
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
    let mut app = MonitorApp::new(config);
    let tick_rate = Duration::from_millis(app.interval_ms);

    // Initial metrics collection
    // Wait for CPU measurement interval
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    app.update_metrics()?;

    let mut last_tick = Instant::now();

    // Main loop
    loop {
        // Draw UI
        terminal.draw(|frame| render_ui(frame, &app))?;

        // Handle events with timeout
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout).context("Event poll failed")? {
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

        // Update metrics on tick
        if last_tick.elapsed() >= tick_rate {
            app.update_metrics()?;
            last_tick = Instant::now();
        }
    }

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
