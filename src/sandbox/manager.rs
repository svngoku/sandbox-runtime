//! Main sandbox manager orchestrator

use crate::config::SandboxRuntimeConfig;
use crate::error::{Result, SandboxError};
use crate::proxy::{HttpProxy, SocksProxy};
use crate::sandbox::violation_store::ViolationStore;
use crate::utils::platform::{get_platform, Platform};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

#[cfg(target_os = "linux")]
use crate::sandbox::linux::LinuxSandbox;

#[cfg(target_os = "macos")]
use crate::sandbox::macos::MacOSSandbox;

use crate::sandbox::docker::DockerSandbox;

/// Sandbox manager
pub struct SandboxManager {
    config: SandboxRuntimeConfig,
    http_proxy: Arc<Mutex<Option<HttpProxy>>>,
    socks_proxy: Arc<Mutex<Option<SocksProxy>>>,
    violation_store: ViolationStore,
    initialized: bool,
}

impl SandboxManager {
    /// Create a new sandbox manager
    pub fn new(config: SandboxRuntimeConfig) -> Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            http_proxy: Arc::new(Mutex::new(None)),
            socks_proxy: Arc::new(Mutex::new(None)),
            violation_store: ViolationStore::new(),
            initialized: false,
        })
    }

    /// Initialize the sandbox manager (start proxies)
    pub async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        info!("Initializing sandbox manager");

        // Start HTTP proxy
        let mut http_proxy = HttpProxy::new(
            self.config.network.allowed_domains.clone(),
            self.config.network.denied_domains.clone(),
        )?;

        let http_port = http_proxy.start().await?;
        info!("HTTP proxy started on port {}", http_port);

        // Start SOCKS5 proxy
        let mut socks_proxy = SocksProxy::new(
            self.config.network.allowed_domains.clone(),
            self.config.network.denied_domains.clone(),
        )?;

        let socks_port = socks_proxy.start().await?;
        info!("SOCKS5 proxy started on port {}", socks_port);

        *self.http_proxy.lock().await = Some(http_proxy);
        *self.socks_proxy.lock().await = Some(socks_proxy);

        self.initialized = true;

        // Start violation monitoring on macOS
        #[cfg(target_os = "macos")]
        {
            if let Err(e) = self.violation_store.start_monitoring() {
                tracing::warn!("Failed to start violation monitoring: {}", e);
            }
        }

        Ok(())
    }

    /// Wrap a command with sandbox
    pub async fn wrap_command(&self, command: &str) -> Result<String> {
        if !self.initialized {
            return Err(SandboxError::Execution(
                "SandboxManager not initialized. Call initialize() first.".to_string(),
            ));
        }

        // If Docker is configured, use Docker sandbox
        if let Some(ref docker_config) = self.config.docker {
            debug!("Using Docker sandbox");
            return Ok(format!(
                "docker run --rm {} {}",
                docker_config.image, command
            ));
        }

        // Otherwise, use OS-level sandbox
        let platform = get_platform();

        match platform {
            #[cfg(target_os = "linux")]
            Platform::Linux => {
                let http_port = self
                    .http_proxy
                    .lock()
                    .await
                    .as_ref()
                    .map(|p| p.port())
                    .ok_or_else(|| SandboxError::Execution("HTTP proxy not started".to_string()))?;

                let socks_port = self
                    .socks_proxy
                    .lock()
                    .await
                    .as_ref()
                    .map(|p| p.port())
                    .ok_or_else(|| SandboxError::Execution("SOCKS proxy not started".to_string()))?;

                let mut sandbox = LinuxSandbox::new(self.config.clone())?;
                sandbox.set_proxy_ports(http_port, socks_port);
                sandbox.wrap_command(command)
            }

            #[cfg(target_os = "macos")]
            Platform::MacOS => {
                let http_port = self
                    .http_proxy
                    .lock()
                    .await
                    .as_ref()
                    .map(|p| p.port())
                    .ok_or_else(|| SandboxError::Execution("HTTP proxy not started".to_string()))?;

                let socks_port = self
                    .socks_proxy
                    .lock()
                    .await
                    .as_ref()
                    .map(|p| p.port())
                    .ok_or_else(|| SandboxError::Execution("SOCKS proxy not started".to_string()))?;

                let mut sandbox = MacOSSandbox::new(self.config.clone())?;
                sandbox.set_proxy_ports(http_port, socks_port);
                sandbox.wrap_command(command)
            }

            _ => Err(SandboxError::UnsupportedPlatform(format!(
                "Platform {} is not supported",
                platform.as_str()
            ))),
        }
    }

    /// Execute a command in the sandbox
    pub async fn execute(&self, command: &str) -> Result<i32> {
        info!("Executing command in sandbox: {}", command);

        // If Docker is configured, use Docker sandbox
        if let Some(ref docker_config) = self.config.docker {
            let mut docker_sandbox = DockerSandbox::new(docker_config.clone()).await?;

            let container_id = docker_sandbox.create_container().await?;
            info!("Created Docker container: {}", container_id);

            docker_sandbox.start_container().await?;

            let exit_code = docker_sandbox.execute_command(command).await?;

            if docker_config.auto_remove {
                docker_sandbox.remove_container().await?;
            }

            return Ok(exit_code);
        }

        // Otherwise, use OS-level sandbox
        let wrapped = self.wrap_command(command).await?;

        let output = crate::utils::exec::execute_shell(&wrapped, true)?;

        Ok(output.status)
    }

    /// Get the violation store
    pub fn violation_store(&self) -> &ViolationStore {
        &self.violation_store
    }

    /// Reset the sandbox manager (stop proxies)
    pub async fn reset(&mut self) -> Result<()> {
        info!("Resetting sandbox manager");

        *self.http_proxy.lock().await = None;
        *self.socks_proxy.lock().await = None;

        self.initialized = false;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{NetworkConfig, FilesystemConfig};

    #[tokio::test]
    async fn test_sandbox_manager_creation() {
        let config = SandboxRuntimeConfig {
            network: NetworkConfig {
                allowed_domains: vec!["*.example.com".to_string()],
                ..Default::default()
            },
            filesystem: FilesystemConfig {
                allow_write: vec![".".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        let manager = SandboxManager::new(config);
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_sandbox_initialization() {
        let config = SandboxRuntimeConfig {
            network: NetworkConfig {
                allowed_domains: vec!["*.example.com".to_string()],
                ..Default::default()
            },
            filesystem: FilesystemConfig {
                allow_write: vec![".".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        let mut manager = SandboxManager::new(config).unwrap();
        let result = manager.initialize().await;
        assert!(result.is_ok());
    }
}
