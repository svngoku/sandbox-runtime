//! macOS sandbox implementation using sandbox-exec

use crate::config::{SandboxRuntimeConfig, FilesystemConfig, NetworkConfig};
use crate::error::{Result, SandboxError};
use crate::utils::exec::command_exists;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info};

/// macOS sandbox using sandbox-exec
pub struct MacOSSandbox {
    config: SandboxRuntimeConfig,
    http_proxy_port: Option<u16>,
    socks_proxy_port: Option<u16>,
}

impl MacOSSandbox {
    /// Create a new macOS sandbox
    pub fn new(config: SandboxRuntimeConfig) -> Result<Self> {
        if !command_exists("sandbox-exec") {
            return Err(SandboxError::UnsupportedPlatform(
                "sandbox-exec is not available on this system".to_string(),
            ));
        }

        Ok(Self {
            config,
            http_proxy_port: None,
            socks_proxy_port: None,
        })
    }

    /// Set proxy ports
    pub fn set_proxy_ports(&mut self, http_port: u16, socks_port: u16) {
        self.http_proxy_port = Some(http_port);
        self.socks_proxy_port = Some(socks_port);
    }

    /// Generate seatbelt profile
    fn generate_profile(&self) -> Result<String> {
        let mut profile = String::from("(version 1)\n");
        profile.push_str("(deny default)\n");
        profile.push_str("(allow process*)\n");
        profile.push_str("(allow sysctl*)\n");
        profile.push_str("(allow mach*)\n");

        // Allow network to proxy ports only
        if let Some(http_port) = self.http_proxy_port {
            profile.push_str(&format!(
                "(allow network* (remote ip \"localhost:{}\"))\n",
                http_port
            ));
        }

        if let Some(socks_port) = self.socks_proxy_port {
            profile.push_str(&format!(
                "(allow network* (remote ip \"localhost:{}\"))\n",
                socks_port
            ));
        }

        // Filesystem rules
        self.add_filesystem_rules(&mut profile)?;

        debug!("Generated seatbelt profile:\n{}", profile);
        Ok(profile)
    }

    /// Add filesystem rules to profile
    fn add_filesystem_rules(&self, profile: &mut String) -> Result<()> {
        let fs_config = &self.config.filesystem;

        // Allow read everywhere by default
        profile.push_str("(allow file-read*)\n");

        // Deny read to specific paths
        for path in &fs_config.deny_read {
            let expanded = expand_path(path)?;
            profile.push_str(&format!(
                "(deny file-read* (subpath \"{}\"))\n",
                expanded.display()
            ));
        }

        // Allow write to specific paths
        for path in &fs_config.allow_write {
            let expanded = expand_path(path)?;
            profile.push_str(&format!(
                "(allow file-write* (subpath \"{}\"))\n",
                expanded.display()
            ));
        }

        // Deny write to specific paths (takes precedence)
        for path in &fs_config.deny_write {
            let expanded = expand_path(path)?;
            profile.push_str(&format!(
                "(deny file-write* (subpath \"{}\"))\n",
                expanded.display()
            ));
        }

        Ok(())
    }

    /// Wrap a command with sandbox
    pub fn wrap_command(&self, command: &str) -> Result<String> {
        let profile = self.generate_profile()?;

        // Save profile to temporary file
        let profile_path = std::env::temp_dir().join(format!(
            "srt-profile-{}.sb",
            std::process::id()
        ));

        std::fs::write(&profile_path, profile)?;

        let wrapped = format!(
            "sandbox-exec -f {} sh -c {}",
            profile_path.display(),
            shell_words::quote(command)
        );

        debug!("Wrapped command: {}", wrapped);
        Ok(wrapped)
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

/// Check if sandbox-exec is available
pub fn is_sandbox_exec_available() -> bool {
    command_exists("sandbox-exec")
}

#[cfg(test)]
#[cfg(target_os = "macos")]
mod tests {
    use super::*;
    use crate::config::FilesystemConfig;

    #[test]
    fn test_macos_sandbox_creation() {
        let config = SandboxRuntimeConfig {
            network: NetworkConfig::default(),
            filesystem: FilesystemConfig {
                allow_write: vec![".".to_string()],
                deny_read: vec!["~/.ssh".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        let sandbox = MacOSSandbox::new(config);
        assert!(sandbox.is_ok());
    }

    #[test]
    fn test_profile_generation() {
        let config = SandboxRuntimeConfig {
            network: NetworkConfig::default(),
            filesystem: FilesystemConfig {
                allow_write: vec![".".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        let mut sandbox = MacOSSandbox::new(config).unwrap();
        sandbox.set_proxy_ports(3128, 1080);

        let profile = sandbox.generate_profile().unwrap();
        assert!(profile.contains("localhost:3128"));
        assert!(profile.contains("localhost:1080"));
    }
}
