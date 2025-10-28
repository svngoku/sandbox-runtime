//! Anthropic Sandbox Runtime
//!
//! A lightweight OS-level sandboxing tool for filesystem and network restrictions.
//! Provides secure-by-default sandboxing without requiring containers.

#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod config;
pub mod error;
pub mod proxy;
pub mod sandbox;
pub mod utils;

pub use config::{NetworkConfig, FilesystemConfig, SandboxRuntimeConfig};
pub use error::{Result, SandboxError};
pub use sandbox::manager::SandboxManager;
pub use sandbox::violation_store::ViolationStore;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
