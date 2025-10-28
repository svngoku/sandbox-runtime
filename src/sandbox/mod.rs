//! Sandbox implementations and management

pub mod manager;
pub mod violation_store;
pub mod linux;
pub mod macos;
pub mod docker;
pub mod seccomp;

pub use manager::SandboxManager;
pub use violation_store::ViolationStore;
