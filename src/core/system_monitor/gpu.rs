use super::metrics::GpuMetrics;
use crate::error::Result;

/// Trait for GPU metrics providers
///
/// This trait abstracts GPU monitoring across different vendors (NVIDIA, AMD, Intel).
/// Implementations are provided in the platform layer.
pub trait GpuProvider: Send {
    /// Get the vendor of the GPU
    fn vendor(&self) -> GpuVendor;

    /// Collect current GPU metrics
    fn collect_metrics(&mut self) -> Result<GpuMetrics>;

    /// Check if the GPU provider is available and functional
    fn is_available(&self) -> bool;
}

// Re-export GpuVendor for convenience
pub use super::metrics::GpuVendor;
