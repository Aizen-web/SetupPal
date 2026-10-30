//! Scripted `OsProvider` for offline unit tests.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;

use crate::config::CoreError;
use crate::os::{CommandOutput, OsProvider};

type Handler = Box<dyn Fn(&Path, &[String], Option<&Path>) -> Option<CommandOutput> + Send + Sync>;

/// A recorded invocation of `run_command_in_dir`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedCommand {
    pub binary: PathBuf,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

/// Mock provider with a fake PATH, a set of "installed" binaries, scripted command
/// outputs, and a log of every command it was asked to run.
#[derive(Default)]
pub struct MockOsProvider {
    path: Mutex<Vec<PathBuf>>,
    binaries: Mutex<HashMap<String, PathBuf>>,
    handlers: Mutex<Vec<Handler>>,
    registry: Mutex<HashMap<String, String>>,
    calls: Mutex<Vec<RecordedCommand>>,
}

pub fn ok(stdout: &str) -> CommandOutput {
    CommandOutput {
        exit_code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

pub fn fail(code: i32, stderr: &str) -> CommandOutput {
    CommandOutput {
        exit_code: Some(code),
        stdout: String::new(),
        stderr: stderr.to_string(),
    }
}

impl MockOsProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes `name` resolvable through `which_in_paths` at `path`.
    pub fn with_binary(self, name: &str, path: impl Into<PathBuf>) -> Self {
        if let Ok(mut b) = self.binaries.lock() {
            b.insert(name.to_string(), path.into());
        }
        self
    }

    pub fn with_path_entry(self, dir: impl Into<PathBuf>) -> Self {
        if let Ok(mut p) = self.path.lock() {
            p.push(dir.into());
        }
        self
    }

    pub fn with_registry(self, key: &str, value: Option<&str>, data: &str) -> Self {
        if let Ok(mut r) = self.registry.lock() {
            r.insert(registry_key(key, value), data.to_string());
        }
        self
    }

    /// Responds to commands whose binary file stem is `binary` and whose arguments start
    /// with `args_prefix`. Earlier registrations win.
    pub fn on(self, binary: &str, args_prefix: &[&str], output: CommandOutput) -> Self {
        let binary = binary.to_ascii_lowercase();
        let prefix: Vec<String> = args_prefix.iter().map(|s| s.to_string()).collect();
        self.on_fn(move |bin, args, _cwd| {
            let stem = bin
                .file_stem()
                .map(|s| s.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if stem == binary && args.len() >= prefix.len() && args[..prefix.len()] == prefix[..] {
                Some(output.clone())
            } else {
                None
            }
        })
    }

    pub fn on_fn(
        self,
        f: impl Fn(&Path, &[String], Option<&Path>) -> Option<CommandOutput> + Send + Sync + 'static,
    ) -> Self {
        if let Ok(mut h) = self.handlers.lock() {
            h.push(Box::new(f));
        }
        self
    }

    pub fn calls(&self) -> Vec<RecordedCommand> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn add_path_entry(&self, dir: PathBuf) {
        if let Ok(mut p) = self.path.lock() {
            p.push(dir);
        }
    }
}

fn registry_key(key: &str, value: Option<&str>) -> String {
    format!("{}|{}", key.to_ascii_lowercase(), value.unwrap_or("").to_ascii_lowercase())
}

#[async_trait]
impl OsProvider for MockOsProvider {
    fn os_name(&self) -> String {
        "windows".to_string()
    }

    async fn os_version(&self) -> String {
        "Mock Windows 11".to_string()
    }

    fn arch(&self) -> String {
        "x86_64".to_string()
    }

    async fn detect_gpu(&self) -> Option<String> {
        None
    }

    async fn free_disk_bytes(&self, _target_dir: &Path) -> u64 {
        0
    }

    async fn refresh_and_get_path(&self) -> Result<Vec<PathBuf>, CoreError> {
        Ok(self.path.lock().map(|p| p.clone()).unwrap_or_default())
    }

    fn which_in_paths(&self, binary_name: &str, search_paths: &[PathBuf]) -> Option<PathBuf> {
        if let Some(found) = self
            .binaries
            .lock()
            .ok()
            .and_then(|b| b.get(binary_name).cloned())
        {
            return Some(found);
        }
        for dir in search_paths {
            for cand in [format!("{binary_name}.exe"), binary_name.to_string()] {
                let full = dir.join(cand);
                if full.is_file() {
                    return Some(full);
                }
            }
        }
        None
    }

    async fn add_to_user_path(&self, directory: &Path) -> Result<String, CoreError> {
        let mut p = self
            .path
            .lock()
            .map_err(|_| CoreError::Os("mock path lock poisoned".to_string()))?;
        let previous = p
            .iter()
            .map(|e| e.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(";");
        if !p.iter().any(|e| e == directory) {
            p.push(directory.to_path_buf());
        }
        Ok(previous)
    }

    async fn restore_user_path(&self, previous_raw: &str) -> Result<(), CoreError> {
        let mut p = self
            .path
            .lock()
            .map_err(|_| CoreError::Os("mock path lock poisoned".to_string()))?;
        *p = previous_raw
            .split(';')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();
        Ok(())
    }

    async fn run_command_in_dir(
        &self,
        binary: &Path,
        args: &[String],
        _extra_path: &[PathBuf],
        cwd: Option<&Path>,
    ) -> Result<CommandOutput, CoreError> {
        if let Ok(mut c) = self.calls.lock() {
            c.push(RecordedCommand {
                binary: binary.to_path_buf(),
                args: args.to_vec(),
                cwd: cwd.map(Path::to_path_buf),
            });
        }
        let handlers = self
            .handlers
            .lock()
            .map_err(|_| CoreError::Os("mock handler lock poisoned".to_string()))?;
        for h in handlers.iter() {
            if let Some(out) = h(binary, args, cwd) {
                return Ok(out);
            }
        }
        Ok(fail(1, &format!("mock: no script for {} {}", binary.display(), args.join(" "))))
    }

    async fn read_registry_string(&self, key: &str, value: Option<&str>) -> Option<String> {
        self.registry
            .lock()
            .ok()
            .and_then(|r| r.get(&registry_key(key, value)).cloned())
    }
}
