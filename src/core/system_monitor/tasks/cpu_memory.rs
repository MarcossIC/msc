//! CPU, Memory, and Process monitoring task.
//!
//! These subsystems are grouped together because they share the sysinfo::System instance.

use sysinfo::{CpuRefreshKind, MemoryRefreshKind, ProcessRefreshKind, RefreshKind, System};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration, MissedTickBehavior};

use super::SubsystemUpdate;
use crate::core::system_monitor::{collect_cpu, collect_memory, sort_and_truncate_processes};

/// Task that monitors CPU, Memory, and Processes.
///
/// Polling frequency: 1 second (base)
/// These metrics are collected together as they share the sysinfo::System instance.
pub async fn cpu_memory_process_task(
    update_tx: mpsc::Sender<SubsystemUpdate>,
    mut shutdown: broadcast::Receiver<()>,
) {
    // log::info!("CPU/Memory/Process monitoring task started");

    // Initialize sysinfo::System with specific refresh options
    let refresh_kind = RefreshKind::nothing()
        .with_cpu(CpuRefreshKind::everything())
        .with_memory(MemoryRefreshKind::everything())
        .with_processes(
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage(),
        );

    let mut system = System::new_with_specifics(refresh_kind);

    // Cache static values (don't change during runtime)
    let total_memory = system.total_memory();

    // Wait for initial CPU measurement
    // log::debug!("Waiting for initial CPU measurement interval");
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    system.refresh_all();

    // Setup interval timer
    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                // Refresh system (sync I/O operation)
                system.refresh_all();

                // Collect CPU and memory metrics (lightweight)
                let cpu = collect_cpu(&system);
                let memory = collect_memory(&system);

                // Collect and sort processes synchronously
                // Processing top 20 is fast enough to do inline
                let processes = sort_and_truncate_processes(
                    system.processes(),
                    total_memory,
                    20
                );

                // Send update
                if let Err(_e) = update_tx
                    .send(SubsystemUpdate::CpuMemoryProcess {
                        cpu,
                        memory,
                        processes,
                    })
                    .await
                {
                    break;
                }

                // log::trace!("CPU/Memory/Process metrics sent");
            }
            _ = shutdown.recv() => {
                // log::info!("CPU/Memory/Process task shutting down");
                break;
            }
        }
    }
}
