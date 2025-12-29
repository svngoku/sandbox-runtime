//! Debug logging utilities

use tracing::{info, debug, warn, error};
use tracing_subscriber::{EnvFilter, fmt};

/// Debug logger for sandbox operations
pub struct DebugLogger;

impl DebugLogger {
    /// Initialize the debug logger
    pub fn init(debug: bool) {
        let filter = if debug {
            EnvFilter::new("sandbox_runtime=debug")
        } else {
            EnvFilter::new("sandbox_runtime=info")
        };

        fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_thread_ids(false)
            .with_line_number(debug)
            .init();
    }

    /// Log info message
    pub fn info(msg: &str) {
        info!("{}", msg);
    }

    /// Log debug message
    pub fn debug(msg: &str) {
        debug!("{}", msg);
    }

    /// Log warning message
    pub fn warn(msg: &str) {
        warn!("{}", msg);
    }

    /// Log error message
    pub fn error(msg: &str) {
        error!("{}", msg);
    }
}
