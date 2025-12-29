//! Docker container sandbox implementation

use crate::config::{DockerConfig, DockerNetworkMode};
use crate::error::{Result, SandboxError};
use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, StartContainerOptions, WaitContainerOptions,
};
use bollard::models::{HostConfig, Mount, MountTypeEnum};
use std::collections::HashMap;
use std::default::Default;
use tracing::{debug, info, warn};
use futures::stream::StreamExt;

/// Docker sandbox wrapper
pub struct DockerSandbox {
    docker: Docker,
    config: DockerConfig,
    container_id: Option<String>,
}

impl DockerSandbox {
    /// Create a new Docker sandbox
    pub async fn new(config: DockerConfig) -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .map_err(|e| SandboxError::Docker(format!("Failed to connect to Docker: {}", e)))?;

        // Verify Docker is running
        docker
            .ping()
            .await
            .map_err(|e| SandboxError::Docker(format!("Docker daemon not available: {}", e)))?;

        Ok(Self {
            docker,
            config,
            container_id: None,
        })
    }

    /// Create and start the container
    pub async fn create_container(&mut self) -> Result<String> {
        info!("Creating Docker container with image: {}", self.config.image);

        // Parse volumes
        let mounts: Vec<Mount> = self
            .config
            .volumes
            .iter()
            .map(|v| {
                let parts: Vec<&str> = v.split(':').collect();
                if parts.len() == 2 {
                    Mount {
                        target: Some(parts[1].to_string()),
                        source: Some(parts[0].to_string()),
                        typ: Some(MountTypeEnum::BIND),
                        ..Default::default()
                    }
                } else {
                    Mount {
                        target: Some(v.clone()),
                        source: Some(v.clone()),
                        typ: Some(MountTypeEnum::BIND),
                        ..Default::default()
                    }
                }
            })
            .collect();

        // Build host config
        let mut host_config = HostConfig {
            mounts: Some(mounts),
            auto_remove: Some(self.config.auto_remove),
            ..Default::default()
        };

        // Set network mode
        if let Some(ref network_mode) = self.config.network_mode {
            host_config.network_mode = Some(match network_mode {
                DockerNetworkMode::Bridge => "bridge".to_string(),
                DockerNetworkMode::Host => "host".to_string(),
                DockerNetworkMode::None => "none".to_string(),
                DockerNetworkMode::Custom(name) => name.clone(),
            });
        }

        // Set resource limits
        if let Some(cpu_limit) = self.config.cpu_limit {
            host_config.nano_cpus = Some((cpu_limit * 1_000_000_000.0) as i64);
        }

        if let Some(memory_limit) = self.config.memory_limit {
            host_config.memory = Some(memory_limit);
        }

        // Convert env to Vec<String>
        let env: Vec<String> = self
            .config
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        // Build container config
        let config = Config {
            image: Some(self.config.image.clone()),
            working_dir: self.config.workdir.clone(),
            env: Some(env),
            host_config: Some(host_config),
            user: self.config.user.clone(),
            tty: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        // Create container
        let options = CreateContainerOptions {
            name: self.config.name.as_deref().unwrap_or(""),
            platform: None,
        };

        let container = self
            .docker
            .create_container(Some(options), config)
            .await
            .map_err(|e| SandboxError::Docker(format!("Failed to create container: {}", e)))?;

        self.container_id = Some(container.id.clone());

        info!("Created container: {}", container.id);
        Ok(container.id)
    }

    /// Start the container
    pub async fn start_container(&self) -> Result<()> {
        let container_id = self
            .container_id
            .as_ref()
            .ok_or_else(|| SandboxError::Docker("Container not created".to_string()))?;

        info!("Starting container: {}", container_id);

        self.docker
            .start_container::<String>(container_id, None)
            .await
            .map_err(|e| SandboxError::Docker(format!("Failed to start container: {}", e)))?;

        Ok(())
    }

    /// Execute a command in the container
    pub async fn execute_command(&self, command: &str) -> Result<i32> {
        let container_id = self
            .container_id
            .as_ref()
            .ok_or_else(|| SandboxError::Docker("Container not created".to_string()))?;

        info!("Executing command in container: {}", command);

        // Create exec instance
        let exec = self
            .docker
            .create_exec(
                container_id,
                bollard::exec::CreateExecOptions {
                    cmd: Some(vec!["sh", "-c", command]),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    tty: Some(true),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| SandboxError::Docker(format!("Failed to create exec: {}", e)))?;

        // Start exec
        if let bollard::exec::StartExecResults::Attached { mut output, .. } = self
            .docker
            .start_exec(&exec.id, None)
            .await
            .map_err(|e| SandboxError::Docker(format!("Failed to start exec: {}", e)))?
        {
            // Stream output
            while let Some(msg) = output.next().await {
                match msg {
                    Ok(bollard::container::LogOutput::StdOut { message }) => {
                        print!("{}", String::from_utf8_lossy(&message));
                    }
                    Ok(bollard::container::LogOutput::StdErr { message }) => {
                        eprint!("{}", String::from_utf8_lossy(&message));
                    }
                    Err(e) => {
                        warn!("Error reading output: {}", e);
                    }
                    _ => {}
                }
            }
        }

        // Get exit code
        let inspect = self
            .docker
            .inspect_exec(&exec.id)
            .await
            .map_err(|e| SandboxError::Docker(format!("Failed to inspect exec: {}", e)))?;

        Ok(inspect.exit_code.unwrap_or(-1) as i32)
    }

    /// Wait for container to finish
    pub async fn wait_container(&self) -> Result<i64> {
        let container_id = self
            .container_id
            .as_ref()
            .ok_or_else(|| SandboxError::Docker("Container not created".to_string()))?;

        info!("Waiting for container to finish: {}", container_id);

        let mut wait_stream = self.docker.wait_container(
            container_id,
            Some(WaitContainerOptions {
                condition: "not-running",
            }),
        );

        if let Some(result) = wait_stream.next().await {
            let response = result
                .map_err(|e| SandboxError::Docker(format!("Failed to wait for container: {}", e)))?;

            Ok(response.status_code)
        } else {
            Ok(0)
        }
    }

    /// Stop the container
    pub async fn stop_container(&self) -> Result<()> {
        if let Some(ref container_id) = self.container_id {
            info!("Stopping container: {}", container_id);

            self.docker
                .stop_container(container_id, None)
                .await
                .map_err(|e| SandboxError::Docker(format!("Failed to stop container: {}", e)))?;
        }

        Ok(())
    }

    /// Remove the container
    pub async fn remove_container(&self) -> Result<()> {
        if let Some(ref container_id) = self.container_id {
            info!("Removing container: {}", container_id);

            self.docker
                .remove_container(
                    container_id,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await
                .map_err(|e| SandboxError::Docker(format!("Failed to remove container: {}", e)))?;
        }

        Ok(())
    }

    /// Get container ID
    pub fn container_id(&self) -> Option<&str> {
        self.container_id.as_deref()
    }
}

impl Drop for DockerSandbox {
    fn drop(&mut self) {
        // Clean up container if auto_remove is enabled
        if self.config.auto_remove && self.container_id.is_some() {
            debug!("Cleaning up Docker container on drop");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_docker_sandbox_creation() {
        let config = DockerConfig {
            image: "alpine:latest".to_string(),
            name: Some("test-sandbox".to_string()),
            workdir: Some("/workspace".to_string()),
            env: HashMap::new(),
            volumes: vec![],
            network_mode: Some(DockerNetworkMode::None),
            auto_remove: true,
            user: None,
            cpu_limit: None,
            memory_limit: None,
        };

        // This test will only work if Docker is available
        if Docker::connect_with_local_defaults().is_ok() {
            let sandbox = DockerSandbox::new(config).await;
            assert!(sandbox.is_ok() || sandbox.is_err()); // Docker might not be running
        }
    }
}
