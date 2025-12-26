//! Temperature monitoring task.

use sysinfo::Components;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration, MissedTickBehavior};

use super::SubsystemUpdate;
use crate::core::system_monitor::collect_temperatures;

/// Task that monitors temperature sensors.
///
/// Polling frequency: 2 seconds (sensors update slowly)
pub async fn temperatures_task(
    update_tx: mpsc::Sender<SubsystemUpdate>,
    mut shutdown: broadcast::Receiver<()>,
) {
    // log::info!("Temperature monitoring task started");

    // Initialize Components instance
    let mut components = Components::new_with_refreshed_list();

    let mut ticker = interval(Duration::from_secs(2));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                // Refresh temperature sensors
                components.refresh(true);

                // Collect metrics using pure function
                let temp_metrics = collect_temperatures(&components);

                if let Err(e) = update_tx.send(SubsystemUpdate::Temperatures(temp_metrics)).await {
                    log::error!("Failed to send temperatures update: {}", e);
                    break;
                }

                // log::trace!("Temperature metrics sent");
            }
            _ = shutdown.recv() => {
                // log::info!("Temperatures task shutting down");
                break;
            }
        }
    }
}
