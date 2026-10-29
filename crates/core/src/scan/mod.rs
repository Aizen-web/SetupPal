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
