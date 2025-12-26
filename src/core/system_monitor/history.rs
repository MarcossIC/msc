use std::collections::VecDeque;

const DEFAULT_HISTORY_SIZE: usize = 60;

/// Circular buffer for storing metric history (for sparklines)
#[derive(Debug, Clone)]
pub struct MetricsHistory {
    capacity: usize,
    pub cpu_usage: VecDeque<f32>,
    pub memory_usage: VecDeque<f32>,
    pub gpu_usage: VecDeque<u32>,
    pub network_rx: VecDeque<u64>,
    pub network_tx: VecDeque<u64>,
}

impl MetricsHistory {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_HISTORY_SIZE)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            cpu_usage: VecDeque::with_capacity(capacity),
            memory_usage: VecDeque::with_capacity(capacity),
            gpu_usage: VecDeque::with_capacity(capacity),
            network_rx: VecDeque::with_capacity(capacity),
            network_tx: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push_cpu(&mut self, value: f32) {
        let capacity = self.capacity;
        Self::push_value(&mut self.cpu_usage, value, capacity);
    }

    pub fn push_memory(&mut self, value: f32) {
        let capacity = self.capacity;
        Self::push_value(&mut self.memory_usage, value, capacity);
    }

    pub fn push_gpu(&mut self, value: u32) {
        let capacity = self.capacity;
        Self::push_value(&mut self.gpu_usage, value, capacity);
    }

    pub fn push_network(&mut self, rx: u64, tx: u64) {
        if self.network_rx.len() >= self.capacity {
            self.network_rx.pop_front();
            self.network_tx.pop_front();
        }
        self.network_rx.push_back(rx);
        self.network_tx.push_back(tx);
    }

    fn push_value<T>(queue: &mut VecDeque<T>, value: T, capacity: usize) {
        if queue.len() >= capacity {
            queue.pop_front();
        }
        queue.push_back(value);
    }

    /// Convert cpu_usage to u64 slice for sparkline widget
    /// Scales values by 10 to preserve decimal precision (0-1000 range)
    pub fn cpu_as_u64(&self) -> Vec<u64> {
        self.cpu_usage.iter().map(|&v| (v * 10.0) as u64).collect()
    }

    /// Convert memory_usage to u64 slice for sparkline widget
    /// Scales values by 10 to preserve decimal precision (0-1000 range)
    pub fn memory_as_u64(&self) -> Vec<u64> {
        self.memory_usage.iter().map(|&v| (v * 10.0) as u64).collect()
    }
}

impl Default for MetricsHistory {
    fn default() -> Self {
        Self::new()
    }
}
