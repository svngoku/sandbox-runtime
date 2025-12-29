//! CLI binary for sandbox runtime

use clap::Parser;
use sandbox_runtime::{
    config::SandboxRuntimeConfig,
    sandbox::manager::SandboxManager,
    utils::debug::DebugLogger,
    VERSION,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "srt")]
#[command(about = "Sandbox Runtime - Lightweight OS-level sandboxing", long_about = None)]
#[command(version = VERSION)]
struct Cli {
    /// Command to run in sandbox
    #[arg(required = true)]
    command: Vec<String>,

    /// Path to settings file
    #[arg(short, long, env = "SRT_SETTINGS")]
    settings: Option<PathBuf>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Docker image to use (overrides config)
    #[arg(long)]
    docker_image: Option<String>,

    /// Docker container name
    #[arg(long)]
    docker_name: Option<String>,

    /// Docker working directory
    #[arg(long)]
    docker_workdir: Option<String>,

    /// Allow specific domains (can be used multiple times)
    #[arg(long = "allow-domain")]
    allowed_domains: Vec<String>,

    /// Deny specific domains (can be used multiple times)
    #[arg(long = "deny-domain")]
    denied_domains: Vec<String>,

    /// Allow write access to paths (can be used multiple times)
    #[arg(long = "allow-write")]
    allow_write: Vec<String>,

    /// Deny read access to paths (can be used multiple times)
    #[arg(long = "deny-read")]
    deny_read: Vec<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize logger
    DebugLogger::init(cli.debug);

    // Run and exit with appropriate code
    let exit_code = run(cli).await.unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        1
    });

    std::process::exit(exit_code);
}

async fn run(cli: Cli) -> sandbox_runtime::Result<i32> {
    // Load configuration
    let mut config = if let Some(settings_path) = cli.settings {
        SandboxRuntimeConfig::from_file(&settings_path)?
    } else {
        let default_path = SandboxRuntimeConfig::default_settings_path();
        if default_path.exists() {
            SandboxRuntimeConfig::from_file(&default_path)?
        } else {
            SandboxRuntimeConfig::default()
        }
    };

    // Override with CLI arguments
    if !cli.allowed_domains.is_empty() {
        config.network.allowed_domains.extend(cli.allowed_domains);
    }

    if !cli.denied_domains.is_empty() {
        config.network.denied_domains.extend(cli.denied_domains);
    }

    if !cli.allow_write.is_empty() {
        config.filesystem.allow_write.extend(cli.allow_write);
    }

    if !cli.deny_read.is_empty() {
        config.filesystem.deny_read.extend(cli.deny_read);
    }

    // Docker configuration from CLI
    if let Some(docker_image) = cli.docker_image {
        use sandbox_runtime::config::DockerConfig;
        use std::collections::HashMap;

        config.docker = Some(DockerConfig {
            image: docker_image,
            name: cli.docker_name,
            workdir: cli.docker_workdir,
            env: HashMap::new(),
            volumes: vec![],
            network_mode: None,
            auto_remove: true,
            user: None,
            cpu_limit: None,
            memory_limit: None,
        });
    }

    // Join command arguments
    let command = cli.command.join(" ");

    // Create and initialize sandbox manager
    let mut manager = SandboxManager::new(config)?;
    manager.initialize().await?;

    // Execute command
    let exit_code = manager.execute(&command).await?;

    // Cleanup
    manager.reset().await?;

    Ok(exit_code)
}
