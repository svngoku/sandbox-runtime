//! Command execution utilities

use std::process::{Command, Stdio};
use crate::error::{Result, SandboxError};

/// Command output
#[derive(Debug)]
pub struct CommandOutput {
    /// Exit status code
    pub status: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
}

impl CommandOutput {
    /// Check if command was successful
    pub fn success(&self) -> bool {
        self.status == 0
    }
}

/// Execute a command and return output
pub fn execute_command(
    program: &str,
    args: &[&str],
    inherit_stdio: bool,
) -> Result<CommandOutput> {
    let mut cmd = Command::new(program);
    cmd.args(args);

    if inherit_stdio {
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = cmd.status()?;
        Ok(CommandOutput {
            status: status.code().unwrap_or(-1),
            stdout: String::new(),
            stderr: String::new(),
        })
    } else {
        let output = cmd.output()?;
        Ok(CommandOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

/// Execute a shell command
pub fn execute_shell(command: &str, inherit_stdio: bool) -> Result<CommandOutput> {
    execute_command("sh", &["-c", command], inherit_stdio)
}

/// Check if a command exists
pub fn command_exists(command: &str) -> bool {
    execute_command("which", &[command], false)
        .map(|output| output.success())
        .unwrap_or(false)
}

/// Get command path
pub fn get_command_path(command: &str) -> Result<String> {
    let output = execute_command("which", &[command], false)?;
    if output.success() {
        Ok(output.stdout.trim().to_string())
    } else {
        Err(SandboxError::CommandNotFound(command.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_command() {
        let output = execute_command("echo", &["hello"], false).unwrap();
        assert!(output.success());
        assert_eq!(output.stdout.trim(), "hello");
    }

    #[test]
    fn test_command_exists() {
        assert!(command_exists("ls"));
        assert!(!command_exists("nonexistent_command_12345"));
    }
}
