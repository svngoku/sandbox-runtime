//! Error types for the sandbox runtime

use std::io;
use thiserror::Error;

/// Result type alias for sandbox operations
pub type Result<T> = std::result::Result<T, SandboxError>;

/// Errors that can occur in the sandbox runtime
#[derive(Error, Debug)]
pub enum SandboxError {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Sandbox execution error
    #[error("Sandbox execution error: {0}")]
    Execution(String),

    /// Platform not supported
    #[error("Platform not supported: {0}")]
    UnsupportedPlatform(String),

    /// Proxy error
    #[error("Proxy error: {0}")]
    Proxy(String),

    /// Docker error
    #[error("Docker error: {0}")]
    Docker(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Command not found
    #[error("Command not found: {0}")]
    CommandNotFound(String),

    /// Violation detected
    #[error("Sandbox violation: {0}")]
    Violation(String),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<String> for SandboxError {
    fn from(s: String) -> Self {
        SandboxError::Other(s)
    }
}

impl From<&str> for SandboxError {
    fn from(s: &str) -> Self {
        SandboxError::Other(s.to_string())
    }
}
