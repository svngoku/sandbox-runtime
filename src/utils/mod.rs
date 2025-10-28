//! Utility modules

pub mod debug;
pub mod exec;
pub mod platform;
pub mod ripgrep;

pub use debug::DebugLogger;
pub use exec::{execute_command, CommandOutput};
pub use platform::{Platform, get_platform, is_command_available};
pub use ripgrep::search_files;
