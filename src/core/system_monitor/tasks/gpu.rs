//! GPU monitoring task.

use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration, MissedTickBehavior};

use super::SubsystemUpdate;
use crate::platform::gpu::get_gpu_provider;

/// Task that monitors GPU metrics.
///
/// Polling frequency: 2 seconds (NVML calls are expensive)
pub async fn gpu_task(
    update_tx: mpsc::Sender<SubsystemUpdate>,
    mut shutdown: broadcast::Receiver<()>,
) {
    // log::info!("GPU monitoring task started");

    // Try to initialize GPU provider (exclusive ownership)
    let mut gpu_provider = match get_gpu_provider() {
        Ok(provider) => provider,
        Err(e) => {
            log::warn!("GPU provider not available: {}", e);
            // Send None and terminate task (no GPU available)
            let _ = update_tx.send(SubsystemUpdate::Gpu(None)).await;
            return;
        }
    };

    // Cache GPU name (static)
    let _gpu_name = gpu_provider
        .collect_metrics()
        .ok()
        .map(|m| m.name.clone())
        .unwrap_or_default();

    let mut ticker = interval(Duration::from_secs(2));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut consecutive_failures = 0;
    const MAX_FAILURES: u32 = 5;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                match gpu_provider.collect_metrics() {
                    Ok(metrics) => {
                        consecutive_failures = 0;
                        if let Err(e) = update_tx.send(SubsystemUpdate::Gpu(Some(metrics))).await {
                            log::error!("Failed to send GPU update: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        log::error!("GPU collection failed (attempt {}): {}", consecutive_failures, e);

                        if consecutive_failures >= MAX_FAILURES {
                            // log::error!("GPU monitoring disabled after {} consecutive failures", MAX_FAILURES);
                            let _ = update_tx.send(SubsystemUpdate::Gpu(None)).await;
                            break;
                        }

                        // Exponential backoff
                        let backoff = Duration::from_secs(2u64.pow(consecutive_failures.min(5)));
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
            _ = shutdown.recv() => {
                // log::info!("GPU task shutting down");
                break;
            }
        }
    }

    // log::info!("GPU monitoring task terminated");
}
