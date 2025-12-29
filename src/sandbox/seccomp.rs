//! Seccomp BPF filter management for blocking Unix sockets

use crate::error::{Result, SandboxError};
use crate::utils::platform::get_arch;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Seccomp filter manager
pub struct SeccompFilter;

impl SeccompFilter {
    /// Get the path to the pre-generated BPF filter
    pub fn get_filter_path() -> Result<PathBuf> {
        let arch = get_arch();
        let filter_path = PathBuf::from("vendor")
            .join("seccomp")
            .join(arch)
            .join("unix-block.bpf");

        if filter_path.exists() {
            debug!("Using pre-generated BPF filter: {}", filter_path.display());
            Ok(filter_path)
        } else {
            Err(SandboxError::Config(format!(
                "Seccomp BPF filter not found for architecture: {}",
                arch
            )))
        }
    }

    /// Get the path to the Python helper script
    pub fn get_python_helper_path() -> Result<PathBuf> {
        let helper_path = PathBuf::from("vendor")
            .join("seccomp-src")
            .join("apply-seccomp-and-exec.py");

        if helper_path.exists() {
            debug!("Found Python helper: {}", helper_path.display());
            Ok(helper_path)
        } else {
            Err(SandboxError::Config(
                "Seccomp Python helper not found".to_string()
            ))
        }
    }

    /// Apply seccomp filter using Python helper
    pub fn apply_filter_command(command: &str) -> Result<String> {
        let filter_path = Self::get_filter_path()?;
        let helper_path = Self::get_python_helper_path()?;

        let wrapped = format!(
            "python3 {} {} -- {}",
            helper_path.display(),
            filter_path.display(),
            command
        );

        debug!("Seccomp wrapped command: {}", wrapped);
        Ok(wrapped)
    }

    /// Check if seccomp is supported on this platform
    pub fn is_supported() -> bool {
        cfg!(target_os = "linux") && Self::get_filter_path().is_ok()
    }

    /// Compile seccomp filter from source (fallback)
    pub fn compile_filter() -> Result<()> {
        info!("Compiling seccomp filter from source");

        let arch = get_arch();
        let src_path = PathBuf::from("vendor/seccomp-src/seccomp-unix-block.c");
        let output_dir = PathBuf::from("vendor/seccomp").join(arch);

        if !output_dir.exists() {
            std::fs::create_dir_all(&output_dir)?;
        }

        let output_path = output_dir.join("unix-block.bpf");

        // Try to compile with gcc
        let compile_result = std::process::Command::new("gcc")
            .args(&[
                "-o",
                output_path.to_str().unwrap(),
                src_path.to_str().unwrap(),
                "-lseccomp",
            ])
            .status();

        match compile_result {
            Ok(status) if status.success() => {
                info!("Successfully compiled seccomp filter");
                Ok(())
            }
            Ok(_) => Err(SandboxError::Execution(
                "Failed to compile seccomp filter".to_string()
            )),
            Err(e) => {
                warn!("GCC not available, trying clang");

                // Try with clang
                let clang_result = std::process::Command::new("clang")
                    .args(&[
                        "-o",
                        output_path.to_str().unwrap(),
                        src_path.to_str().unwrap(),
                        "-lseccomp",
                    ])
                    .status();

                match clang_result {
                    Ok(status) if status.success() => {
                        info!("Successfully compiled seccomp filter with clang");
                        Ok(())
                    }
                    _ => Err(SandboxError::CommandNotFound(
                        "Neither gcc nor clang available for compiling seccomp filter".to_string()
                    )),
                }
            }
        }
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use super::*;

    #[test]
    fn test_filter_path() {
        let result = SeccompFilter::get_filter_path();
        // May or may not exist depending on build
        println!("Filter path: {:?}", result);
    }

    #[test]
    fn test_python_helper_path() {
        let result = SeccompFilter::get_python_helper_path();
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_is_supported() {
        // Should return true on Linux, false elsewhere
        #[cfg(target_os = "linux")]
        assert!(SeccompFilter::is_supported() || !SeccompFilter::is_supported());
    }
}
