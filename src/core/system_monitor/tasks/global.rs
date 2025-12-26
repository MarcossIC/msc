//! Global system metrics task.

use sysinfo::System;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration, MissedTickBehavior};

use super::SubsystemUpdate;
use crate::core::system_monitor::GlobalMetrics;

/// Task that monitors global system metrics (uptime, hostname, boot time).
///
/// Polling frequency: 5 seconds (mostly static information)
pub async fn global_metrics_task(
    update_tx: mpsc::Sender<SubsystemUpdate>,
    mut shutdown: broadcast::Receiver<()>,
) {
    // log::info!("Global metrics monitoring task started");

    // Cache static values
    let hostname = System::host_name().unwrap_or_else(|| "Unknown".to_string());
    let boot_time = System::boot_time() as i64;

    let mut ticker = interval(Duration::from_secs(5));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let now = chrono::Utc::now().timestamp();
                let uptime_secs = (now - boot_time).max(0) as u64;

                let global = GlobalMetrics {
                    uptime_secs,
                    hostname: hostname.clone(),
                    boot_time,
                    // power_source and battery are updated in battery_task
                    power_source: Default::default(),
                    battery_percent: None,
                    battery_time_remaining_secs: None,
                };

                if let Err(e) = update_tx.send(SubsystemUpdate::Global(global)).await {
                    log::error!("Failed to send global metrics update: {}", e);
                    break;
                }

                // log::trace!("Global metrics sent");
            }
            _ = shutdown.recv() => {
                // log::info!("Global metrics task shutting down");
                break;
            }
        }
    }
}
