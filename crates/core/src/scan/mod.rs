use std::path::Path;
use std::sync::Arc;

use crate::config::CoreError;
use crate::events::{AgentEvent, EventBus};
use crate::os::OsProvider;
use crate::types::{AllowedBinary, DetectedTool, SystemProfile};

/// Scans the host machine for OS, architecture, GPU, free disk space,
/// active PATH (refreshing from Registry on Windows), and allowlisted developer tools.
pub struct SystemScanner {
    os: Arc<dyn OsProvider>,
    events: EventBus,
}

impl SystemScanner {
    pub fn new(os: Arc<dyn OsProvider>, events: EventBus) -> Self {
        Self { os, events }
    }

    pub async fn scan(&self) -> Result<SystemProfile, CoreError> {
        self.events.emit(AgentEvent::ScanStarted);

        let os_name = self.os.os_name();
        let os_version = self.os.os_version().await;
        let arch = self.os.arch();
        let gpu_detected = self.os.detect_gpu().await;
        let free_disk_bytes = self.os.free_disk_bytes(Path::new(".")).await;
        let active_path_entries = self.os.refresh_and_get_path().await?;

        let mut installed_tools = Vec::new();
        for &binary in AllowedBinary::ALL {
            let exe_name = binary.executable_name();
            if let Some(resolved_path) = self.os.which_in_paths(exe_name, &active_path_entries) {
                let args = vec![binary.version_flag().to_string()];
                if let Ok(cmd_out) = self
                    .os
                    .run_command(&resolved_path, &args, &active_path_entries)
                    .await
                {
                    let combined = if !cmd_out.stdout.is_empty() {
                        cmd_out.stdout
                    } else {
                        cmd_out.stderr
                    };
                    let first_line = combined
                        .lines()
                        .next()
                        .unwrap_or("installed")
                        .trim()
                        .to_string();

                    installed_tools.push(DetectedTool {
                        binary,
                        version_output: first_line,
                        path: resolved_path,
                    });
                }
            }
        }

        let profile = SystemProfile {
            os_name,
            os_version,
            arch,
            gpu_detected,
            free_disk_bytes,
            installed_tools,
            active_path_entries,
        };

        self.events.emit(AgentEvent::ScanCompleted {
            profile: Box::new(profile.clone()),
        });

        Ok(profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::CommandOutput;
    use async_trait::async_trait;
    use std::path::PathBuf;

    struct FakeOsProvider;

    #[async_trait]
    impl OsProvider for FakeOsProvider {
        fn os_name(&self) -> String {
            "windows".to_string()
        }
        async fn os_version(&self) -> String {
            "Windows 11 Pro 23H2".to_string()
        }
        fn arch(&self) -> String {
            "x86_64".to_string()
        }
        async fn detect_gpu(&self) -> Option<String> {
            Some("NVIDIA GeForce RTX 4080".to_string())
        }
        async fn free_disk_bytes(&self, _target_dir: &Path) -> u64 {
            128 * 1024 * 1024 * 1024
        }
        async fn refresh_and_get_path(&self) -> Result<Vec<PathBuf>, CoreError> {
            Ok(vec![PathBuf::from(r"C:\\Program Files\\Git\\cmd")])
        }
        fn which_in_paths(&self, binary_name: &str, _search_paths: &[PathBuf]) -> Option<PathBuf> {
            if binary_name == "git" || binary_name == "winget" {
                Some(PathBuf::from(format!(r"C:\\Windows\\System32\\{binary_name}.exe")))
            } else {
                None
            }
        }
        async fn run_command(
            &self,
            binary: &Path,
            _args: &[String],
            _extra_path: &[PathBuf],
        ) -> Result<CommandOutput, CoreError> {
            Ok(CommandOutput {
                exit_code: Some(0),
                stdout: format!("{} version 2.46.0", binary.display()),
                stderr: String::new(),
            })
        }
    }

    #[tokio::test]
    async fn scanner_builds_system_profile_and_emits_events() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();
        let scanner = SystemScanner::new(Arc::new(FakeOsProvider), bus);

        let profile = scanner.scan().await.expect("scan should succeed");
        assert_eq!(profile.os_name, "windows");
        assert_eq!(profile.gpu_detected.as_deref(), Some("NVIDIA GeForce RTX 4080"));
        assert_eq!(profile.installed_tools.len(), 2);

        let ev1 = rx.recv().await.expect("ScanStarted event");
        assert!(matches!(ev1, AgentEvent::ScanStarted));
        let ev2 = rx.recv().await.expect("ScanCompleted event");
        assert!(matches!(ev2, AgentEvent::ScanCompleted { .. }));
    }
}
