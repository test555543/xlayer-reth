//! Block performance metrics module
//!
//! This module provides performance monitoring for ExEx stage block processing.
//! Duration precision is adaptive:
//! - >= 1 microsecond: recorded in microseconds
//! - < 1 microsecond: recorded in nanoseconds
//!
//! This ensures that even very fast operations have meaningful, non-zero values.

use std::time::{Duration, Instant};

use metrics::histogram;

/// Record duration with adaptive precision
///
/// This function converts Duration to the appropriate unit:
/// - If duration >= 1 microsecond: returns microseconds (f64)
/// - If duration < 1 microsecond: returns nanoseconds (f64)
///
/// This ensures that even very fast operations (< 1μs) have meaningful values.
#[inline]
fn duration_to_adaptive_precision(duration: Duration) -> (f64, &'static str) {
    let nanos = duration.as_nanos() as f64;
    
    // If >= 1 microsecond, use microseconds
    if nanos >= 1_000.0 {
        (nanos / 1_000.0, "microseconds")
    } else {
        // Otherwise, use nanoseconds for sub-microsecond precision
        (nanos, "nanoseconds")
    }
}

/// Record a block performance metric with block number
///
/// # Arguments
/// * `metric_name` - The metric name (e.g., "xlayer.block.exex.replay_execution.duration")
/// * `duration` - The duration to record
/// * `block_number` - The block number (as u64)
/// * `stage` - The stage name ("exex" or "io")
/// * `operation` - Optional operation type (e.g., "replay_execution", "state_provider", "batch_write")
///
/// # Example
/// ```
/// record_block_metric(
///     "xlayer.block.exex.replay_execution.duration",
///     duration,
///     12345,
///     "exex",
///     Some("replay_execution"),
/// );
/// ```
pub fn record_block_metric(
    metric_name: &'static str,
    duration: Duration,
    block_number: u64,
    stage: &str,
    operation: Option<&str>,
) {
    let (duration_value, unit) = duration_to_adaptive_precision(duration);
    
    // The metrics crate's histogram! macro is used to register a histogram, then use .record() to record the value
    // Macro format: histogram!(name, key1 => value1, key2 => value2, ...)
    // Note: Label values must be String type, not &str (because 'static lifetime is required)
    let block_number_str = block_number.to_string();
    let unit_str = unit.to_string();
    let stage_str = stage.to_string();
    
    if let Some(op) = operation {
        let op_str = op.to_string();
        // Register histogram using macro, then record the value
        histogram!(
            metric_name,
            "block_number" => block_number_str,
            "stage" => stage_str,
            "unit" => unit_str,
            "operation" => op_str
        )
        .record(duration_value);
    } else {
        histogram!(
            metric_name,
            "block_number" => block_number_str,
            "stage" => stage_str,
            "unit" => unit_str
        )
        .record(duration_value);
    }
}


/// A guard that records the duration when dropped
///
/// This provides an elegant way to measure duration using RAII pattern.
///
/// # Example
/// ```
/// let _guard = BlockMetricGuard::new(
///     "xlayer.block.exex.replay_execution.duration",
///     block_number,
///     "exex",
///     Some("replay_execution"),
/// );
/// // ... do work ...
/// // Guard automatically records duration when dropped
/// ```
pub struct BlockMetricGuard {
    metric_name: &'static str,
    block_number: u64,
    stage: &'static str,
    operation: Option<&'static str>,
    start: Instant,
}

impl BlockMetricGuard {
    /// Create a new metric guard
    pub fn new(
        metric_name: &'static str,
        block_number: u64,
        stage: &'static str,
        operation: Option<&'static str>,
    ) -> Self {
        Self {
            metric_name,
            block_number,
            stage,
            operation,
            start: Instant::now(),
        }
    }
    
    /// Manually record the metric (normally called on drop)
    pub fn record(self) {
        let duration = self.start.elapsed();
        record_block_metric(
            self.metric_name,
            duration,
            self.block_number,
            self.stage,
            self.operation,
        );
    }
}

impl Drop for BlockMetricGuard {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        record_block_metric(
            self.metric_name,
            duration,
            self.block_number,
            self.stage,
            self.operation,
        );
    }
}


/// Metric name constants (only for ExEx stage - the only stage we can instrument)
pub mod metric_names {
    // ExEx Handler metrics
    pub const EXEX_STATE_PROVIDER_CREATION: &str = "xlayer.block.exex.state_provider.creation.duration";
    pub const EXEX_DB_STATE_BUILD: &str = "xlayer.block.exex.db_state.build.duration";
    pub const EXEX_INSPECTOR_INIT: &str = "xlayer.block.exex.inspector.init.duration";
    pub const EXEX_EVM_ENV_SETUP: &str = "xlayer.block.exex.evm_env.setup.duration";
    pub const EXEX_EXECUTOR_CREATION: &str = "xlayer.block.exex.executor.creation.duration";
    pub const EXEX_REPLAY_EXECUTION: &str = "xlayer.block.exex.replay_execution.duration";
    pub const EXEX_DB_WRITE: &str = "xlayer.block.exex.db.write.duration";
    pub const EXEX_TOTAL: &str = "xlayer.block.exex.total.duration";
}

/// Stage name constants
pub mod stage_names {
    pub const EXEX: &str = "exex";
}

/// Operation name constants (only for ExEx stage operations)
pub mod operation_names {
    pub const STATE_PROVIDER: &str = "state_provider";
    pub const REPLAY_EXECUTION: &str = "replay_execution";
    pub const BATCH_WRITE: &str = "batch_write";
}
