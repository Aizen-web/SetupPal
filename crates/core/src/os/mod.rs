pub mod mock;
pub mod unix;
pub mod windows;

pub use mock::MockOsProvider;

use std::path::{Path, PathBuf};
use async_trait::async_trait;
use crate::config::CoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Abstracts OS-specific operations (Windows Registry PATH refresh, disk space, GPU probe,
/// binary resolution, and process execution) so Windows (`winget`), macOS (`brew`), and
/// Linux (`apt`) share a unified interface and can be mocked in unit tests.
#[async_trait]
pub trait OsProvider: Send + Sync {
    fn os_name(&self) -> String;
    async fn os_version(&self) -> String;
    fn arch(&self) -> String;
    async fn detect_gpu(&self) -> Option<String>;
    async fn free_disk_bytes(&self, target_dir: &Path) -> u64;

    /// Reads the freshest User + System PATH entries.
    /// On Windows, queries `HKCU\\Environment` and
    /// `HKLM\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment`
    /// so newly installed tools are visible without restarting the shell.
    async fn refresh_and_get_path(&self) -> Result<Vec<PathBuf>, CoreError>;

    /// Resolves an executable name against the provided `search_paths`.
    fn which_in_paths(&self, binary_name: &str, search_paths: &[PathBuf]) -> Option<PathBuf>;

    /// Adds a directory to User PATH and refreshes process PATH. Returns previous raw PATH.
    async fn add_to_user_path(&self, directory: &Path) -> Result<String, CoreError>;

    /// Restores User PATH to a previous raw value.
    async fn restore_user_path(&self, previous_raw: &str) -> Result<(), CoreError>;

    /// Runs a binary directly (never via a shell interpreter) with explicit arguments.
    async fn run_command(
        &self,
        binary: &Path,
        args: &[String],
        extra_path: &[PathBuf],
    ) -> Result<CommandOutput, CoreError>;
}

/// Default platform provider that delegates to Windows or Unix implementations.
#[derive(Debug, Clone, Default)]
pub struct PlatformOsProvider;

impl PlatformOsProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl OsProvider for PlatformOsProvider {
    fn os_name(&self) -> String {
        std::env::consts::OS.to_string()
    }

    async fn os_version(&self) -> String {
        if cfg!(target_os = "windows") {
            windows::query_windows_version().await
        } else {
            unix::query_unix_version().await
        }
    }

    fn arch(&self) -> String {
        std::env::consts::ARCH.to_string()
    }

    async fn detect_gpu(&self) -> Option<String> {
        if cfg!(target_os = "windows") {
            windows::detect_windows_gpu().await
        } else {
            unix::detect_unix_gpu().await
        }
    }

    async fn free_disk_bytes(&self, target_dir: &Path) -> u64 {
        if cfg!(target_os = "windows") {
            windows::query_windows_free_disk(target_dir).await
        } else {
            unix::query_unix_free_disk(target_dir).await
        }
    }

    async fn refresh_and_get_path(&self) -> Result<Vec<PathBuf>, CoreError> {
        if cfg!(target_os = "windows") {
            windows::read_windows_registry_path().await
        } else {
            Ok(unix::read_unix_env_path())
        }
    }

    fn which_in_paths(&self, binary_name: &str, search_paths: &[PathBuf]) -> Option<PathBuf> {
        let candidates: Vec<String> = if cfg!(target_os = "windows") {
            vec![
                format!("{binary_name}.exe"),
                format!("{binary_name}.cmd"),
                format!("{binary_name}.bat"),
                binary_name.to_string(),
            ]
        } else {
            vec![binary_name.to_string()]
        };

        for dir in search_paths {
            for cand in &candidates {
                let full = dir.join(cand);
                if full.is_file() {
                    return Some(full);
                }
            }
        }
        None
    }

    async fn add_to_user_path(&self, directory: &Path) -> Result<String, CoreError> {
        if cfg!(target_os = "windows") {
            windows::add_to_windows_user_path(directory).await
        } else {
            unix::add_to_unix_user_path(directory).await
        }
    }

    async fn restore_user_path(&self, previous_raw: &str) -> Result<(), CoreError> {
        if cfg!(target_os = "windows") {
            windows::restore_windows_user_path(previous_raw).await
        } else {
            unix::restore_unix_user_path(previous_raw).await
        }
    }

    async fn run_command(
        &self,
        binary: &Path,
        args: &[String],
        extra_path: &[PathBuf],
    ) -> Result<CommandOutput, CoreError> {
        let mut cmd = tokio::process::Command::new(binary);
        cmd.args(args);
        cmd.stdin(std::process::Stdio::null());

        if !extra_path.is_empty() {
            if let Ok(joined) = std::env::join_paths(extra_path) {
                cmd.env("PATH", joined);
            }
        }

        // Hard execution timeout (5 minutes) to prevent runaway or hanging processes
        let timeout_duration = std::time::Duration::from_secs(300);
        let out = match tokio::time::timeout(timeout_duration, cmd.output()).await {
            Ok(output_res) => output_res?,
            Err(_) => {
                return Err(CoreError::Os(format!(
                    "Command '{}' timed out after {} seconds",
                    binary.display(),
                    timeout_duration.as_secs()
                )));
            }
        };

        Ok(CommandOutput {
            exit_code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        })
    }
}
