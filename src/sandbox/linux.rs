//! Linux sandbox implementation using bubblewrap

use crate::config::{SandboxRuntimeConfig, FilesystemConfig};
use crate::error::{Result, SandboxError};
use crate::utils::exec::{command_exists, get_command_path};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, info};

/// Linux sandbox using bubblewrap
pub struct LinuxSandbox {
    config: SandboxRuntimeConfig,
    bwrap_path: String,
    socat_path: Option<String>,
    python_path: Option<String>,
    http_proxy_port: Option<u16>,
    socks_proxy_port: Option<u16>,
}

impl LinuxSandbox {
    /// Create a new Linux sandbox
    pub fn new(config: SandboxRuntimeConfig) -> Result<Self> {
        // Check for required dependencies
        if !command_exists("bwrap") {
            return Err(SandboxError::CommandNotFound(
                "bubblewrap (bwrap) is not installed".to_string(),
            ));
        }

        let bwrap_path = get_command_path("bwrap")?;
        let socat_path = if command_exists("socat") {
            Some(get_command_path("socat")?)
        } else {
            None
        };

        let python_path = if command_exists("python3") {
            Some(get_command_path("python3")?)
        } else {
            None
        };

        Ok(Self {
            config,
            bwrap_path,
            socat_path,
            python_path,
            http_proxy_port: None,
            socks_proxy_port: None,
        })
    }

    /// Set proxy ports
    pub fn set_proxy_ports(&mut self, http_port: u16, socks_port: u16) {
        self.http_proxy_port = Some(http_port);
        self.socks_proxy_port = Some(socks_port);
    }

    /// Wrap a command with sandbox
    pub fn wrap_command(&self, command: &str) -> Result<String> {
        let mut args = Vec::new();

        // Network isolation
        args.push("--unshare-net".to_string());
        args.push("--unshare-ipc".to_string());

        // Filesystem isolation
        self.add_filesystem_args(&mut args)?;

        // Environment variables
        if let Some(http_port) = self.http_proxy_port {
            args.push("--setenv".to_string());
            args.push("HTTP_PROXY".to_string());
            args.push(format!("http://localhost:{}", http_port));
            args.push("--setenv".to_string());
            args.push("HTTPS_PROXY".to_string());
            args.push(format!("http://localhost:{}", http_port));
        }

        // Add the command to execute
        args.push("sh".to_string());
        args.push("-c".to_string());
        args.push(command.to_string());

        // Build the final command
        let wrapped = format!("{} {}", self.bwrap_path, shell_words::join(&args));

        debug!("Wrapped command: {}", wrapped);
        Ok(wrapped)
    }

    /// Add filesystem arguments
    fn add_filesystem_args(&self, args: &mut Vec<String>) -> Result<()> {
        let fs_config = &self.config.filesystem;

        // Bind root as read-only by default
        args.push("--ro-bind".to_string());
        args.push("/".to_string());
        args.push("/".to_string());

        // Allow write access to specified directories
        for path in &fs_config.allow_write {
            let expanded = expand_path(path)?;
            if expanded.exists() {
                args.push("--bind".to_string());
                args.push(expanded.to_string_lossy().to_string());
                args.push(expanded.to_string_lossy().to_string());
            }
        }

        // Create tmpfs for /tmp
        args.push("--tmpfs".to_string());
        args.push("/tmp".to_string());

        // Bind /dev
        args.push("--dev".to_string());
        args.push("/dev".to_string());

        // Bind /proc
        args.push("--proc".to_string());
        args.push("/proc".to_string());

        Ok(())
    }

    /// Execute a command in the sandbox
    pub fn execute(&self, command: &str) -> Result<i32> {
        let wrapped = self.wrap_command(command)?;

        info!("Executing sandboxed command");

        let status = Command::new("sh")
            .arg("-c")
            .arg(&wrapped)
            .status()?;

        Ok(status.code().unwrap_or(-1))
    }
}

/// Expand path with shell expansion
fn expand_path(path: &str) -> Result<PathBuf> {
    let expanded = shellexpand::full(path)
        .map_err(|e| SandboxError::Config(format!("Failed to expand path {}: {}", path, e)))?;

    Ok(PathBuf::from(expanded.as_ref()))
}

/// Check if bubblewrap is available
pub fn is_bubblewrap_available() -> bool {
    command_exists("bwrap")
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use super::*;
    use crate::config::{NetworkConfig, FilesystemConfig};

    #[test]
    fn test_linux_sandbox_creation() {
        if is_bubblewrap_available() {
            let config = SandboxRuntimeConfig {
                network: NetworkConfig::default(),
                filesystem: FilesystemConfig {
                    allow_write: vec![".".to_string()],
                    ..Default::default()
                },
                ..Default::default()
            };

            let sandbox = LinuxSandbox::new(config);
            assert!(sandbox.is_ok());
        }
    }

    #[test]
    fn test_path_expansion() {
        let expanded = expand_path("~/.ssh").unwrap();
        assert!(expanded.to_string_lossy().contains(".ssh"));
    }
}
