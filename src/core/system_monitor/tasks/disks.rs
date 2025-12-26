//! Disk monitoring task.

use std::sync::Arc;
use sysinfo::Disks;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration, Instant, MissedTickBehavior};

use super::SubsystemUpdate;
use crate::core::system_monitor::{collect_disks, disk_enrichment::get_disk_enrichment_provider};

/// Task that monitors disk usage.
///
/// Two-tier polling strategy:
/// - Basic usage stats (sysinfo): every 3 seconds
/// - Extended SMART data (PowerShell): every 30 seconds
pub async fn disks_task(
    update_tx: mpsc::Sender<SubsystemUpdate>,
    mut shutdown: broadcast::Receiver<()>,
) {
    // log::info!("Disks monitoring task started");

    // Initialize Disks instance and enrichment provider
    let mut disks = Disks::new_with_refreshed_list();
    let enrichment_provider = Arc::new(get_disk_enrichment_provider());

    let mut ticker = interval(Duration::from_secs(3));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    // Track last enrichment time
    let mut last_enrichment = Instant::now();
    let enrichment_interval = Duration::from_secs(30);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                // Refresh disk information
                disks.refresh(true);

                // Collect basic metrics using pure function
                let mut disk_metrics = collect_disks(&disks);

                // Enrich with extended data if interval elapsed
                if last_enrichment.elapsed() >= enrichment_interval {
                    // Clone metrics for fallback in case enrichment fails
                    let fallback = disk_metrics.clone();

                    // Clone Arc for use in blocking task
                    let provider = Arc::clone(&enrichment_provider);

                    // Run enrichment in blocking thread to avoid blocking Tokio runtime
                    disk_metrics = tokio::task::spawn_blocking(move || {
                        provider.enrich_disks(disk_metrics)
                    })
                    .await
                    .unwrap_or(fallback);

                    last_enrichment = Instant::now();
                    // log::debug!("Disk metrics enriched with SMART data");
                }

                if let Err(_e) = update_tx.send(SubsystemUpdate::Disks(disk_metrics)).await {
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
