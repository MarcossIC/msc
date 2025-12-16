//! Battery monitoring task.

use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration};

use super::SubsystemUpdate;
use crate::core::system_monitor::collect_battery_info;

/// Task that monitors battery status and power source.
///
/// Polling frequency: 10 seconds (battery changes slowly)
pub async fn battery_task(
    update_tx: mpsc::Sender<SubsystemUpdate>,
    mut shutdown: broadcast::Receiver<()>,
) {
    // log::info!("Battery monitoring task started");

    let mut ticker = interval(Duration::from_secs(10));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let (power_source, battery_percent, battery_time_remaining) =
                    collect_battery_info();

                if let Err(e) = update_tx.send(SubsystemUpdate::Battery {
                    power_source,
                    battery_percent,
                    battery_time_remaining,
                }).await {
                    log::error!("Failed to send battery update: {}", e);
                    break;
                }
            }
            _ = shutdown.recv() => {
                // log::info!("Battery task shutting down");
                break;
            }
        }
    }
}
