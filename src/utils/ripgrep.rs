//! Ripgrep wrapper for fast file searching

use crate::error::Result;
use crate::utils::exec::execute_command;

/// Search for files matching a pattern
pub fn search_files(pattern: &str, directory: &str) -> Result<Vec<String>> {
    let output = execute_command(
        "rg",
        &["--files", "--glob", pattern, directory],
        false,
    )?;

    if output.success() {
        Ok(output
            .stdout
            .lines()
            .map(|s| s.to_string())
            .collect())
    } else {
        Ok(Vec::new())
    }
}

/// Search for content in files
pub fn search_content(pattern: &str, directory: &str) -> Result<Vec<String>> {
    let output = execute_command(
        "rg",
        &["-l", pattern, directory],
        false,
    )?;

    if output.success() {
        Ok(output
            .stdout
            .lines()
            .map(|s| s.to_string())
            .collect())
    } else {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_files() {
        // This test requires ripgrep to be installed
        if crate::utils::exec::command_exists("rg") {
            let result = search_files("*.rs", ".");
            assert!(result.is_ok());
        }
    }
}
