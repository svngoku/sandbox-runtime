//! Sandbox violation monitoring and storage

use crate::error::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};

/// Violation types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationType {
    /// Network access violation
    Network,
    /// Filesystem read violation
    FilesystemRead,
    /// Filesystem write violation
    FilesystemWrite,
    /// Unix socket violation
    UnixSocket,
    /// Other violation
    Other,
}

/// A sandbox violation
#[derive(Debug, Clone)]
pub struct Violation {
    /// Type of violation
    pub violation_type: ViolationType,
    /// Target of the violation (e.g., domain, file path)
    pub target: String,
    /// Process that caused the violation
    pub process: String,
    /// Timestamp
    pub timestamp: std::time::SystemTime,
}

/// Violation store for tracking sandbox violations
pub struct ViolationStore {
    violations: Arc<Mutex<Vec<Violation>>>,
    subscribers: Arc<Mutex<Vec<Box<dyn Fn(&Violation) + Send + Sync>>>>,
}

impl ViolationStore {
    /// Create a new violation store
    pub fn new() -> Self {
        Self {
            violations: Arc::new(Mutex::new(Vec::new())),
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a violation
    pub fn add_violation(&self, violation: Violation) {
        debug!("Recording violation: {:?}", violation);

        // Store violation
        {
            let mut violations = self.violations.lock().unwrap();
            violations.push(violation.clone());
        }

        // Notify subscribers
        {
            let subscribers = self.subscribers.lock().unwrap();
            for subscriber in subscribers.iter() {
                subscriber(&violation);
            }
        }
    }

    /// Subscribe to violations
    pub fn subscribe<F>(&self, callback: F)
    where
        F: Fn(&Violation) + Send + Sync + 'static,
    {
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.push(Box::new(callback));
    }

    /// Get all violations
    pub fn get_violations(&self) -> Vec<Violation> {
        let violations = self.violations.lock().unwrap();
        violations.clone()
    }

    /// Get violations by type
    pub fn get_violations_by_type(&self, violation_type: ViolationType) -> Vec<Violation> {
        let violations = self.violations.lock().unwrap();
        violations
            .iter()
            .filter(|v| v.violation_type == violation_type)
            .cloned()
            .collect()
    }

    /// Clear all violations
    pub fn clear(&self) {
        let mut violations = self.violations.lock().unwrap();
        violations.clear();
    }

    /// Get violation count
    pub fn count(&self) -> usize {
        let violations = self.violations.lock().unwrap();
        violations.len()
    }

    /// Start monitoring violations (macOS only)
    #[cfg(target_os = "macos")]
    pub fn start_monitoring(&self) -> Result<()> {
        use std::process::Command;

        info!("Starting violation monitoring on macOS");

        let store = self.clone();

        std::thread::spawn(move || {
            // Monitor sandbox violations using log stream
            let mut child = Command::new("log")
                .args(&[
                    "stream",
                    "--predicate",
                    "subsystem == 'com.apple.sandbox'",
                    "--style",
                    "syslog",
                ])
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to start log stream");

            if let Some(stdout) = child.stdout.take() {
                use std::io::{BufRead, BufReader};

                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        if line.contains("deny") {
                            store.parse_and_add_violation(&line);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Parse and add violation from log line
    fn parse_and_add_violation(&self, line: &str) {
        debug!("Parsing violation: {}", line);

        let violation_type = if line.contains("file-read") {
            ViolationType::FilesystemRead
        } else if line.contains("file-write") {
            ViolationType::FilesystemWrite
        } else if line.contains("network") {
            ViolationType::Network
        } else if line.contains("unix-socket") {
            ViolationType::UnixSocket
        } else {
            ViolationType::Other
        };

        // Extract target from log line (simplified)
        let target = line
            .split_whitespace()
            .last()
            .unwrap_or("unknown")
            .to_string();

        let violation = Violation {
            violation_type,
            target,
            process: "sandboxed-process".to_string(),
            timestamp: std::time::SystemTime::now(),
        };

        self.add_violation(violation);
    }
}

impl Clone for ViolationStore {
    fn clone(&self) -> Self {
        Self {
            violations: Arc::clone(&self.violations),
            subscribers: Arc::clone(&self.subscribers),
        }
    }
}

impl Default for ViolationStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_violation_store() {
        let store = ViolationStore::new();

        let violation = Violation {
            violation_type: ViolationType::Network,
            target: "evil.com".to_string(),
            process: "test".to_string(),
            timestamp: std::time::SystemTime::now(),
        };

        store.add_violation(violation);

        assert_eq!(store.count(), 1);
        assert_eq!(
            store.get_violations_by_type(ViolationType::Network).len(),
            1
        );

        store.clear();
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_subscriber() {
        let store = ViolationStore::new();
        let called = Arc::new(Mutex::new(false));
        let called_clone = Arc::clone(&called);

        store.subscribe(move |_| {
            let mut c = called_clone.lock().unwrap();
            *c = true;
        });

        let violation = Violation {
            violation_type: ViolationType::FilesystemWrite,
            target: "/etc/passwd".to_string(),
            process: "test".to_string(),
            timestamp: std::time::SystemTime::now(),
        };

        store.add_violation(violation);

        assert!(*called.lock().unwrap());
    }
}
