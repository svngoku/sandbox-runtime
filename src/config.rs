//! Configuration types and validation for sandbox runtime

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main sandbox runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SandboxRuntimeConfig {
    /// Network restrictions
    pub network: NetworkConfig,

    /// Filesystem restrictions
    pub filesystem: FilesystemConfig,

    /// Docker container configuration (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker: Option<DockerConfig>,

    /// Violations to ignore per command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_violations: Option<HashMap<String, Vec<String>>>,

    /// Enable weaker nested sandbox (for running inside containers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_weaker_nested_sandbox: Option<bool>,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    /// Allowed domains (supports wildcards like *.example.com)
    #[serde(default)]
    pub allowed_domains: Vec<String>,

    /// Denied domains (takes precedence over allowed)
    #[serde(default)]
    pub denied_domains: Vec<String>,

    /// Allowed Unix socket paths (macOS)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_unix_sockets: Option<Vec<String>>,

    /// Allow all Unix sockets (Linux - disables seccomp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_all_unix_sockets: Option<bool>,

    /// Allow binding to local ports
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_local_binding: Option<bool>,
}

/// Filesystem configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemConfig {
    /// Paths to deny read access
    #[serde(default)]
    pub deny_read: Vec<String>,

    /// Paths to allow write access
    #[serde(default)]
    pub allow_write: Vec<String>,

    /// Paths to deny write access (takes precedence)
    #[serde(default)]
    pub deny_write: Vec<String>,
}

/// Docker container configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerConfig {
    /// Docker image to use
    pub image: String,

    /// Container name (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Working directory inside container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Volumes to mount (host_path:container_path)
    #[serde(default)]
    pub volumes: Vec<String>,

    /// Network mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_mode: Option<DockerNetworkMode>,

    /// Remove container after execution
    #[serde(default = "default_true")]
    pub auto_remove: bool,

    /// User to run as (uid:gid)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// CPU limit (0.0 = unlimited)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_limit: Option<f64>,

    /// Memory limit in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_limit: Option<i64>,
}

/// Docker network modes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockerNetworkMode {
    /// Bridge network (default)
    Bridge,
    /// Host network
    Host,
    /// No network
    None,
    /// Custom network
    Custom(String),
}

fn default_true() -> bool {
    true
}

impl SandboxRuntimeConfig {
    /// Load configuration from a file
    pub fn from_file(path: &PathBuf) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a file
    pub fn to_file(&self, path: &PathBuf) -> crate::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> crate::Result<()> {
        // Validate network config
        if self.network.allowed_domains.is_empty() && self.network.denied_domains.is_empty() {
            tracing::warn!("No network restrictions configured");
        }

        // Validate filesystem config
        if self.filesystem.allow_write.is_empty() {
            tracing::warn!("No write permissions configured");
        }

        Ok(())
    }

    /// Get default settings path
    pub fn default_settings_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".srt-settings.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = SandboxRuntimeConfig {
            network: NetworkConfig {
                allowed_domains: vec!["*.example.com".to_string()],
                denied_domains: vec!["evil.com".to_string()],
                ..Default::default()
            },
            filesystem: FilesystemConfig {
                deny_read: vec!["~/.ssh".to_string()],
                allow_write: vec![".".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: SandboxRuntimeConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.network.allowed_domains, parsed.network.allowed_domains);
        assert_eq!(config.network.denied_domains, parsed.network.denied_domains);
    }

    #[test]
    fn test_docker_config() {
        let config = SandboxRuntimeConfig {
            docker: Some(DockerConfig {
                image: "ubuntu:22.04".to_string(),
                name: Some("test-sandbox".to_string()),
                workdir: Some("/workspace".to_string()),
                env: HashMap::new(),
                volumes: vec!["/tmp:/tmp".to_string()],
                network_mode: Some(DockerNetworkMode::None),
                auto_remove: true,
                user: Some("1000:1000".to_string()),
                cpu_limit: Some(1.0),
                memory_limit: Some(512 * 1024 * 1024),
            }),
            ..Default::default()
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("ubuntu:22.04"));
    }
}
