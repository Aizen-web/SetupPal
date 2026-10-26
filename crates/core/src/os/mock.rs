use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use async_trait::async_trait;

use crate::config::CoreError;
use crate::os::{CommandOutput, OsProvider};

/// Fully configurable in-memory `OsProvider` for zero-destructive unit tests.
#[derive(Debug, Default)]
pub struct MockOsProvider {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub gpu: Option<String>,
    pub free_disk: u64,
    pub paths: Mutex<Vec<PathBuf>>,
    pub binaries: Mutex<HashMap<String, PathBuf>>,
    pub command_handlers: Mutex<HashMap<String, CommandOutput>>,
    pub added_paths: Mutex<Vec<PathBuf>>,
}

impl MockOsProvider {
    pub fn new() -> Self {
        Self {
            os_name: "windows".to_string(),
            os_version: "Windows 11 Pro".to_string(),
            arch: "x86_64".to_string(),
            gpu: Some("NVIDIA GeForce RTX 4090".to_string()),
            free_disk: 250_000_000_000,
            paths: Mutex::new(vec![
                PathBuf::from(r"C:\Windows\System32"),
                PathBuf::from(r"C:\Program Files\Git\cmd"),
            ]),
            binaries: Mutex::new(HashMap::new()),
            command_handlers: Mutex::new(HashMap::new()),
            added_paths: Mutex::new(Vec::new()),
        }
    }

    /// Pre-configures a working C++ toolchain (clang++ and cmake) with working version outputs and successful compilation simulation.
    pub fn with_mock_cpp_toolchain() -> Self {
        let mock = Self::new();
        mock.register_binary("winget", PathBuf::from(r"C:\Windows\System32\winget.exe"));
        mock.register_binary("clang++", PathBuf::from(r"C:\Program Files\LLVM\bin\clang++.exe"));
        mock.register_binary("clang", PathBuf::from(r"C:\Program Files\LLVM\bin\clang.exe"));
        mock.register_binary("cmake", PathBuf::from(r"C:\Program Files\CMake\bin\cmake.exe"));

        mock.register_command_output(
            "clang++",
            CommandOutput {
                exit_code: Some(0),
                stdout: "clang version 18.1.8 (target x86_64-pc-windows-msvc)".to_string(),
                stderr: String::new(),
            },
        );
        mock.register_command_output(
            "cmake",
            CommandOutput {
                exit_code: Some(0),
                stdout: "cmake version 3.30.2".to_string(),
                stderr: String::new(),
            },
        );
        mock.register_command_output(
            "hello.exe",
            CommandOutput {
                exit_code: Some(0),
                stdout: "Zero-Trust C++20 Verification Passed! Sum=15".to_string(),
                stderr: String::new(),
            },
        );
        mock
    }

    pub fn register_binary(&self, name: &str, path: PathBuf) {
        if let Ok(mut map) = self.binaries.lock() {
            map.insert(name.to_ascii_lowercase(), path);
        }
    }

    pub fn register_command_output(&self, binary_name: &str, output: CommandOutput) {
        if let Ok(mut map) = self.command_handlers.lock() {
            map.insert(binary_name.to_ascii_lowercase(), output);
        }
    }
}

#[async_trait]
impl OsProvider for MockOsProvider {
    fn os_name(&self) -> String {
        self.os_name.clone()
    }

    async fn os_version(&self) -> String {
        self.os_version.clone()
    }

    fn arch(&self) -> String {
        self.arch.clone()
    }

    async fn detect_gpu(&self) -> Option<String> {
        self.gpu.clone()
    }

    async fn free_disk_bytes(&self, _target_dir: &Path) -> u64 {
        self.free_disk
    }

    async fn refresh_and_get_path(&self) -> Result<Vec<PathBuf>, CoreError> {
        let p = self.paths.lock().map_err(|_| CoreError::Os("lock poisoned".to_string()))?;
        Ok(p.clone())
    }

    fn which_in_paths(&self, binary_name: &str, _search_paths: &[PathBuf]) -> Option<PathBuf> {
        let map = self.binaries.lock().ok()?;
        map.get(&binary_name.to_ascii_lowercase()).cloned()
    }

    async fn add_to_user_path(&self, directory: &Path) -> Result<String, CoreError> {
        let mut p = self.paths.lock().map_err(|_| CoreError::Os("lock poisoned".to_string()))?;
        let old = p.iter().map(|d| d.to_string_lossy().to_string()).collect::<Vec<_>>().join(";");
        if !p.contains(&directory.to_path_buf()) {
            p.push(directory.to_path_buf());
        }
        if let Ok(mut added) = self.added_paths.lock() {
            added.push(directory.to_path_buf());
        }
        Ok(old)
    }

    async fn restore_user_path(&self, previous_raw: &str) -> Result<(), CoreError> {
        let mut p = self.paths.lock().map_err(|_| CoreError::Os("lock poisoned".to_string()))?;
        p.clear();
        for s in previous_raw.split(';').filter(|s| !s.is_empty()) {
            p.push(PathBuf::from(s));
        }
        Ok(())
    }

    async fn run_command(
        &self,
        binary: &Path,
        _args: &[String],
        _extra_path: &[PathBuf],
    ) -> Result<CommandOutput, CoreError> {
        let file_name = binary
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        let clean_name = file_name.trim_end_matches(".exe");

        let map = self.command_handlers.lock().map_err(|_| CoreError::Os("lock poisoned".to_string()))?;
        if let Some(out) = map.get(&file_name).or_else(|| map.get(clean_name)) {
            return Ok(out.clone());
        }

        // Default fallback mock execution (success with binary name)
        Ok(CommandOutput {
            exit_code: Some(0),
            stdout: format!("{} output OK", binary.display()),
            stderr: String::new(),
        })
    }
}
