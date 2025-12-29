//! Platform detection utilities

use std::process::Command;

/// Supported platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS
    MacOS,
    /// Linux
    Linux,
    /// Windows (not yet supported)
    Windows,
    /// Unknown platform
    Unknown,
}

/// Get the current platform
pub fn get_platform() -> Platform {
    #[cfg(target_os = "macos")]
    return Platform::MacOS;

    #[cfg(target_os = "linux")]
    return Platform::Linux;

    #[cfg(target_os = "windows")]
    return Platform::Windows;

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return Platform::Unknown;
}

impl Platform {
    /// Check if the platform is macOS
    pub fn is_macos(&self) -> bool {
        matches!(self, Platform::MacOS)
    }

    /// Check if the platform is Linux
    pub fn is_linux(&self) -> bool {
        matches!(self, Platform::Linux)
    }

    /// Check if the platform is supported
    pub fn is_supported(&self) -> bool {
        matches!(self, Platform::MacOS | Platform::Linux)
    }

    /// Get platform name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::MacOS => "macos",
            Platform::Linux => "linux",
            Platform::Windows => "windows",
            Platform::Unknown => "unknown",
        }
    }
}

/// Check if a command is available in PATH
pub fn is_command_available(command: &str) -> bool {
    Command::new("which")
        .arg(command)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Get the architecture
pub fn get_arch() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    return "x64";

    #[cfg(target_arch = "aarch64")]
    return "arm64";

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    return "unknown";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = get_platform();
        assert!(platform.is_supported() || platform == Platform::Windows);
    }

    #[test]
    fn test_arch_detection() {
        let arch = get_arch();
        assert!(arch == "x64" || arch == "arm64" || arch == "unknown");
    }
}
