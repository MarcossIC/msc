//! Disk monitoring task.

use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration, MissedTickBehavior};
use sysinfo::Disks;

use super::SubsystemUpdate;
use crate::core::system_monitor::collect_disks;

/// Task that monitors disk usage.
///
/// Polling frequency: 3 seconds (disk usage changes slowly)
pub async fn disks_task(
    update_tx: mpsc::Sender<SubsystemUpdate>,
    mut shutdown: broadcast::Receiver<()>,
) {
    // log::info!("Disks monitoring task started");

    // Initialize Disks instance
    let mut disks = Disks::new_with_refreshed_list();

    let mut ticker = interval(Duration::from_secs(3));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                // Refresh disk information
                disks.refresh(true);

                // Collect metrics using pure function
                let disk_metrics = collect_disks(&disks);

                if let Err(e) = update_tx.send(SubsystemUpdate::Disks(disk_metrics)).await {
                    log::error!("Failed to send disks update: {}", e);
                    break;
                }

                // log::trace!("Disk metrics sent");
            }
            _ = shutdown.recv() => {
                // log::info!("Disks task shutting down");
                break;
            }
        }
    }
}
