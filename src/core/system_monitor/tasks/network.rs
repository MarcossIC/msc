//! Network monitoring task.

use sysinfo::Networks;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration, Instant, MissedTickBehavior};

use super::SubsystemUpdate;
use crate::core::system_monitor::NetworkMetrics;

/// Task that monitors network interfaces.
///
/// Polling frequency: 1 second (needed for accurate rate calculation)
pub async fn network_task(
    update_tx: mpsc::Sender<SubsystemUpdate>,
    mut shutdown: broadcast::Receiver<()>,
) {
    // log::info!("Network monitoring task started");

    // Initialize Networks instance
    let mut networks = Networks::new_with_refreshed_list();

    // State tracking for rate calculation
    let mut last_update: Option<Instant> = None;
    let mut last_values: Vec<(u64, u64)> = Vec::new(); // (rx, tx) per interface

    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                // Refresh network stats
                networks.refresh(true);

                let now = Instant::now();
                let elapsed_secs = last_update
                    .map(|t| now.duration_since(t).as_secs_f64())
                    .unwrap_or(1.0);

                // Collect current values
                let current_values: Vec<_> = networks
                    .values()
                    .map(|data| (data.total_received(), data.total_transmitted()))
                    .collect();

                // Calculate metrics with rates
                let metrics: Vec<NetworkMetrics> = networks
                    .iter()
                    .enumerate()
                    .map(|(i, (name, data))| {
                        let (prev_rx, prev_tx) = last_values
                            .get(i)
                            .copied()
                            .unwrap_or((data.total_received(), data.total_transmitted()));

                        let rx_diff = data.total_received().saturating_sub(prev_rx);
                        let tx_diff = data.total_transmitted().saturating_sub(prev_tx);

                        NetworkMetrics {
                            interface: name.to_string(),
                            rx_bytes_total: data.total_received(),
                            tx_bytes_total: data.total_transmitted(),
                            rx_bytes_per_sec: (rx_diff as f64 / elapsed_secs) as u64,
                            tx_bytes_per_sec: (tx_diff as f64 / elapsed_secs) as u64,
                            rx_packets: data.packets_received(),
                            tx_packets: data.packets_transmitted(),
                            rx_errors: data.errors_on_received(),
                            tx_errors: data.errors_on_transmitted(),
                            rx_drops: 0, // sysinfo doesn't provide
                            tx_drops: 0,
                        }
                    })
                    .collect();

                // Update state
                last_update = Some(now);
                last_values = current_values;

                // Send update
                if let Err(_e) = update_tx.send(SubsystemUpdate::Network(metrics)).await {
                    break;
                }

                // log::trace!("Network metrics sent");
            }
            _ = shutdown.recv() => {
                // log::info!("Network task shutting down");
                break;
            }
        }
    }
}
