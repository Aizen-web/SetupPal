#[cfg(test)]
pub mod mock;
pub mod job;
pub mod unix;
pub mod windows;


use std::path::{Path, PathBuf};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
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

    /// Raw User PATH string from the registry (or process env on non-Windows).
    async fn read_user_path_raw(&self) -> Result<String, CoreError> {
        Ok(String::new())
    }

    /// Writes the raw User PATH string (used to strip entries this install added).
    async fn write_user_path_raw(&self, raw: &str) -> Result<(), CoreError> {
        self.restore_user_path(raw).await
    }

    /// Runs a binary directly (never via a shell interpreter) with explicit arguments
    /// in the process working directory.
    async fn run_command(
        &self,
        binary: &Path,
        args: &[String],
        extra_path: &[PathBuf],
    ) -> Result<CommandOutput, CoreError> {
        self.run_command_cancellable(binary, args, extra_path, None, None)
            .await
    }

    /// Runs a binary directly (never via a shell interpreter) with explicit arguments.
    /// `cwd` sets the child's working directory; `None` inherits the current one.
    async fn run_command_in_dir(
        &self,
        binary: &Path,
        args: &[String],
        extra_path: &[PathBuf],
        cwd: Option<&Path>,
    ) -> Result<CommandOutput, CoreError> {
        self.run_command_cancellable(binary, args, extra_path, cwd, None)
            .await
    }

    /// Like `run_command_in_dir`, but aborts the child when `cancel` fires (`kill_on_drop`).
    async fn run_command_cancellable(
        &self,
        binary: &Path,
        args: &[String],
        extra_path: &[PathBuf],
        cwd: Option<&Path>,
        cancel: Option<&CancellationToken>,
    ) -> Result<CommandOutput, CoreError>;

    /// Same as `run_command_cancellable`, with optional stall progress every 90 seconds.
    async fn run_command_with_options(
        &self,
        binary: &Path,
        args: &[String],
        extra_path: &[PathBuf],
        cwd: Option<&Path>,
        cancel: Option<&CancellationToken>,
        on_progress: Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>,
    ) -> Result<CommandOutput, CoreError> {
        let _ = on_progress;
        self.run_command_cancellable(binary, args, extra_path, cwd, cancel)
            .await
    }

    /// Reads a string value from the Windows registry (`value: None` reads the default value).
    /// Returns `None` on other platforms or when the key or value does not exist.
    async fn read_registry_string(&self, _key: &str, _value: Option<&str>) -> Option<String> {
        None
    }
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

    async fn read_user_path_raw(&self) -> Result<String, CoreError> {
        if cfg!(target_os = "windows") {
            windows::read_user_path_raw().await
        } else {
            Ok(std::env::var("PATH").unwrap_or_default())
        }
    }

    async fn write_user_path_raw(&self, raw: &str) -> Result<(), CoreError> {
        self.restore_user_path(raw).await
    }

    async fn read_registry_string(&self, key: &str, value: Option<&str>) -> Option<String> {
        if cfg!(target_os = "windows") {
            windows::query_registry_string(key, value).await
        } else {
            None
        }
    }

    async fn run_command_cancellable(
        &self,
        binary: &Path,
        args: &[String],
        extra_path: &[PathBuf],
        cwd: Option<&Path>,
        cancel: Option<&CancellationToken>,
    ) -> Result<CommandOutput, CoreError> {
        self.run_command_with_options(binary, args, extra_path, cwd, cancel, None)
            .await
    }

    async fn run_command_with_options(
        &self,
        binary: &Path,
        args: &[String],
        extra_path: &[PathBuf],
        cwd: Option<&Path>,
        cancel: Option<&CancellationToken>,
        on_progress: Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>,
    ) -> Result<CommandOutput, CoreError> {
        let mut cmd = tokio::process::Command::new(binary);
        cmd.args(args);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        if !extra_path.is_empty() {
            if let Ok(joined) = std::env::join_paths(extra_path) {
                cmd.env("PATH", joined);
            }
        }

        // Job Object kills the whole tree (msiexec, unins000, …) on timeout/cancel/drop.
        let job = job::KillOnCloseJob::create();
        let child = cmd.spawn()?;
        if let Some(ref job) = job {
            #[cfg(windows)]
            {
                if let Some(handle) = child.raw_handle() {
                    let _ = job.assign_process(handle);
                }
            }
            #[cfg(not(windows))]
            {
                let _ = job;
            }
        }

        let timeout_duration = std::time::Duration::from_secs(300);
        let stall = std::time::Duration::from_secs(90);
        let label = binary.display().to_string();
        let short_name = binary
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("installer");
        let started = std::time::Instant::now();

        let mut join = tokio::spawn(async move {
            let result = tokio::time::timeout(timeout_duration, child.wait_with_output()).await;
            drop(job);
            result
        });

        let timed = loop {
            tokio::select! {
                biased;
                _ = async {
                    if let Some(token) = cancel {
                        token.cancelled().await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    join.abort();
                    let _ = (&mut join).await;
                    return Err(CoreError::Os(format!("Command '{label}' was cancelled")));
                }
                result = &mut join => {
                    break match result {
                        Ok(inner) => inner,
                        Err(_) => {
                            return Err(CoreError::Os(format!("Command '{label}' task failed")));
                        }
                    };
                }
                _ = tokio::time::sleep(stall) => {
                    if let Some(cb) = &on_progress {
                        let secs = started.elapsed().as_secs();
                        cb(format!(
                            "Still working: {short_name} is downloading or waiting on Windows ({secs}s elapsed)"
                        ));
                    }
                }
            }
        };

        let out = match timed {
            Ok(output_res) => output_res?,
            Err(_) => {
                return Err(CoreError::Os(format!(
                    "Command '{label}' timed out after {} seconds",
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
