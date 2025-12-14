//! Terminal User Interface for system monitoring.
//!
//! Provides a real-time dashboard using ratatui.

mod app;
mod event_handler;
mod render;
mod widgets;

pub use app::{run_monitor_app, MonitorApp, MonitorAppConfig};
pub use event_handler::MonitorEvent;
