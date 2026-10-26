pub mod unix;
pub mod windows;

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

    async fn run_command(
        &self,
        binary: &Path,
        args: &[String],
        extra_path: &[PathBuf],
    ) -> Result<CommandOutput, CoreError> {
        let mut cmd = tokio::process::Command::new(binary);
        cmd.args(args);

        if !extra_path.is_empty() {
            if let Ok(joined) = std::env::join_paths(extra_path) {
                cmd.env("PATH", joined);
            }
        }

        let out = cmd.output().await?;
        Ok(CommandOutput {
            exit_code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        })
    }
}
