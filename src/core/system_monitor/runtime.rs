//! Tokio runtime and orchestrator for metrics collection.
//!
//! This module provides the async runtime that coordinates all metrics collection tasks.

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch};

use super::metrics::SystemMetrics;
use super::tasks::{
    battery_task, cpu_memory_process_task, disks_task, global_metrics_task, gpu_task, network_task,
    temperatures_task, SubsystemUpdate,
};

/// Wrapper around the Tokio runtime for metrics collection.
///
/// This provides a clean interface for managing the background metrics collection.
pub struct MetricsRuntime {
    /// Receiver for SystemMetrics snapshots
    pub snapshot_rx: watch::Receiver<Arc<SystemMetrics>>,

    /// Sender for UI state changes (for adaptive scheduling - future)
    pub ui_events_tx: watch::Sender<UiState>,

    /// Shutdown signal sender
    shutdown_tx: broadcast::Sender<()>,

    /// Handle to the runtime (for shutdown)
    _runtime_handle: tokio::runtime::Runtime,
}

/// UI state that tasks can react to (for adaptive scheduling).
#[derive(Debug, Clone)]
pub struct UiState {
    pub selected_tab: usize,
    pub should_collect_processes: bool,
}

impl MetricsRuntime {
    /// Create a new MetricsRuntime with all background tasks spawned.
    pub fn new() -> anyhow::Result<Self> {
        // log::info!("Initializing MetricsRuntime");

        // Create Tokio runtime with 2 worker threads
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_time()
            .thread_name("metrics-worker")
            .build()?;

        // Create channels
        let (snapshot_tx, snapshot_rx) = watch::channel(Arc::new(SystemMetrics::default()));
        let (ui_events_tx, ui_events_rx) = watch::channel(UiState {
            selected_tab: 0,
            should_collect_processes: true,
        });
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        // Spawn all tasks on the runtime
        let shutdown_for_spawn = shutdown_tx.clone();
        runtime.spawn(async move {
            spawn_all_tasks(snapshot_tx, ui_events_rx, shutdown_for_spawn.subscribe()).await
        });

        // log::info!("MetricsRuntime initialized successfully");

        Ok(Self {
            snapshot_rx,
            ui_events_tx,
            shutdown_tx,
            _runtime_handle: runtime,
        })
    }

    /// Shutdown the runtime gracefully.
    pub fn shutdown(self) {
        // log::info!("Shutting down MetricsRuntime");
        let _ = self.shutdown_tx.send(());
        // Runtime will shutdown when dropped
    }
}

/// Spawn all metrics collection tasks.
///
/// This function creates the orchestrator and all subsystem tasks.
pub async fn spawn_all_tasks(
    snapshot_tx: watch::Sender<Arc<SystemMetrics>>,
    _ui_events_rx: watch::Receiver<UiState>,
    shutdown: broadcast::Receiver<()>,
) {
    // log::info!("Spawning all metrics collection tasks");

    // Create mpsc channel for subsystem updates
    let (update_tx, update_rx) = mpsc::channel::<SubsystemUpdate>(32);

    // Spawn orchestrator task
    tokio::spawn(orchestrator_task(
        update_rx,
        snapshot_tx,
        shutdown.resubscribe(),
    ));

    // Spawn subsystem tasks
    tokio::spawn(battery_task(update_tx.clone(), shutdown.resubscribe()));

    tokio::spawn(cpu_memory_process_task(
        update_tx.clone(),
        shutdown.resubscribe(),
    ));

    tokio::spawn(gpu_task(update_tx.clone(), shutdown.resubscribe()));

    tokio::spawn(disks_task(update_tx.clone(), shutdown.resubscribe()));

    tokio::spawn(network_task(update_tx.clone(), shutdown.resubscribe()));

    tokio::spawn(temperatures_task(update_tx.clone(), shutdown.resubscribe()));

    tokio::spawn(global_metrics_task(
        update_tx.clone(),
        shutdown.resubscribe(),
    ));

    // log::info!("All metrics collection tasks spawned");
}

/// Orchestrator task that merges updates from subsystems into complete snapshots.
///
/// This is the heart of the metrics collection system. It receives partial updates
/// from individual subsystem tasks and merges them into a complete SystemMetrics
/// snapshot that is sent to the UI via a watch channel.
async fn orchestrator_task(
    mut update_rx: mpsc::Receiver<SubsystemUpdate>,
    snapshot_tx: watch::Sender<Arc<SystemMetrics>>,
    mut shutdown: broadcast::Receiver<()>,
) {
    // log::info!("Orchestrator task started");

    let mut current_snapshot = SystemMetrics::default();

    loop {
        tokio::select! {
            Some(update) = update_rx.recv() => {
                // Merge update into current snapshot
                match update {
                    SubsystemUpdate::CpuMemoryProcess { cpu, memory, processes } => {
                        current_snapshot.cpu = cpu;
                        current_snapshot.memory = memory;
                        current_snapshot.top_processes = processes;
                    }
                    SubsystemUpdate::Gpu(gpu) => {
                        current_snapshot.gpu = gpu;
                    }
                    SubsystemUpdate::Disks(disks) => {
                        current_snapshot.disks = disks;
                    }
                    SubsystemUpdate::Network(network) => {
                        current_snapshot.network = network;
                    }
                    SubsystemUpdate::Temperatures(temps) => {
                        current_snapshot.temperatures = temps;
                    }
                    SubsystemUpdate::Battery { power_source, battery_percent, battery_time_remaining } => {
                        current_snapshot.global.power_source = power_source;
                        current_snapshot.global.battery_percent = battery_percent;
                        current_snapshot.global.battery_time_remaining_secs = battery_time_remaining;
                    }
                    SubsystemUpdate::Global(global) => {
                        current_snapshot.global = global;
                    }
                }

                // Update timestamp
                current_snapshot.timestamp = chrono::Utc::now().timestamp();

                // Send updated snapshot
                // watch::send() only fails if there are no receivers (which is fine)
                let _ = snapshot_tx.send(Arc::new(current_snapshot.clone()));

                // log::trace!("Snapshot updated and sent to UI");
            }
            _ = shutdown.recv() => {
                // log::info!("Orchestrator task shutting down");
                break;
            }
        }
    }
}
