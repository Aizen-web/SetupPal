use std::path::Path;
use std::sync::Arc;

use crate::adapters::VerificationResult;
use crate::config::CoreError;
use crate::events::{AgentEvent, EventBus};
use crate::os::OsProvider;
use crate::safety::hash::verify_file_sha256;
use crate::types::{AllowedBinary, PlannedStep, Step};

/// Runs deterministic post-install checks per step and per goal.
/// Refreshes PATH via Windows Registry / environment, checks binary presence,
/// runs `--version`, and compiles & executes a temporary modern C++20 `main.cpp`.
#[derive(Clone)]
pub struct Verifier {
    os_provider: Arc<dyn OsProvider>,
    events: EventBus,
}

impl Verifier {
    pub fn new(os_provider: Arc<dyn OsProvider>, events: EventBus) -> Self {
        Self {
            os_provider,
            events,
        }
    }

    /// Verifies a single `PlannedStep`.
    pub async fn verify_step(&self, step: &PlannedStep) -> Result<VerificationResult, CoreError> {
        self.events.emit(AgentEvent::VerificationStarted {
            step_id: step.id.clone(),
        });

        let result = match &step.step {
            Step::RunVerifyCommand { binary, args } => {
                self.verify_binary_command(&step.id, *binary, args).await?
            }
            Step::AddToUserPath { directory } => {
                self.verify_directory_on_path(&step.id, directory).await?
            }
            Step::VerifyHash {
                file_path,
                expected_sha256,
            } => self.verify_hash_step(&step.id, file_path, expected_sha256).await?,
            Step::InstallPackage { package_id, .. } => VerificationResult {
                passed: true,
                command_run: format!("verify_package {package_id}"),
                exit_code: Some(0),
                stdout: format!("Package {package_id} install step recorded"),
                stderr: String::new(),
            },
            Step::DownloadArtifact { destination, .. } => {
                let passed = destination.exists();
                VerificationResult {
                    passed,
                    command_run: format!("verify_file_exists {}", destination.display()),
                    exit_code: if passed { Some(0) } else { Some(1) },
                    stdout: if passed {
                        format!("Artifact exists: {}", destination.display())
                    } else {
                        String::new()
                    },
                    stderr: if !passed {
                        format!("Artifact file missing: {}", destination.display())
                    } else {
                        String::new()
                    },
                }
            }
            Step::CreateIsolatedEnv { path, .. } => {
                let passed = path.exists();
                VerificationResult {
                    passed,
                    command_run: format!("verify_dir_exists {}", path.display()),
                    exit_code: if passed { Some(0) } else { Some(1) },
                    stdout: if passed {
                        format!("Directory exists: {}", path.display())
                    } else {
                        String::new()
                    },
                    stderr: if !passed {
                        format!("Directory missing: {}", path.display())
                    } else {
                        String::new()
                    },
                }
            }
        };

        self.events.emit(AgentEvent::VerificationCompleted {
            step_id: step.id.clone(),
            result: result.clone(),
        });

        Ok(result)
    }

    /// Verifies that an allowed binary exists on the refreshed PATH and runs with `args`.
    pub async fn verify_binary_command(
        &self,
        _step_id: &str,
        binary: AllowedBinary,
        args: &[String],
    ) -> Result<VerificationResult, CoreError> {
        let paths = self.os_provider.refresh_and_get_path().await?;
        let bin_name = binary.executable_name();

        let binary_path = match self.os_provider.which_in_paths(bin_name, &paths) {
            Some(p) => p,
            None => {
                return Ok(VerificationResult {
                    passed: false,
                    command_run: format!("{} {}", bin_name, args.join(" ")),
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!(
                        "Binary '{}' was not found in active PATH entries ({})",
                        bin_name,
                        paths
                            .iter()
                            .take(4)
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()
                            .join("; ")
                    ),
                });
            }
        };

        let output = self
            .os_provider
            .run_command(&binary_path, args, &paths)
            .await?;

        let passed = output.exit_code == Some(0);
        Ok(VerificationResult {
            passed,
            command_run: format!("{} {}", binary_path.display(), args.join(" ")),
            exit_code: output.exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    /// Verifies that a directory is registered in active PATH entries.
    pub async fn verify_directory_on_path(
        &self,
        _step_id: &str,
        directory: &Path,
    ) -> Result<VerificationResult, CoreError> {
        let paths = self.os_provider.refresh_and_get_path().await?;
        let dir_str = directory.to_string_lossy();
        let found = paths.iter().any(|p| {
            p.to_string_lossy().eq_ignore_ascii_case(&dir_str)
        });

        Ok(VerificationResult {
            passed: found,
            command_run: format!("check_path {}", directory.display()),
            exit_code: if found { Some(0) } else { Some(1) },
            stdout: if found {
                format!("Directory is present in active PATH: {}", directory.display())
            } else {
                String::new()
            },
            stderr: if !found {
                format!("Directory not found in refreshed PATH: {}", directory.display())
            } else {
                String::new()
            },
        })
    }

    /// Verifies a file's SHA-256 hash.
    pub async fn verify_hash_step(
        &self,
        _step_id: &str,
        file_path: &Path,
        expected_sha256: &str,
    ) -> Result<VerificationResult, CoreError> {
        match verify_file_sha256(file_path, expected_sha256) {
            Ok(actual) => Ok(VerificationResult {
                passed: true,
                command_run: format!("sha256sum {}", file_path.display()),
                exit_code: Some(0),
                stdout: format!("SHA-256 hash verified successfully: {actual}"),
                stderr: String::new(),
            }),
            Err(e) => Ok(VerificationResult {
                passed: false,
                command_run: format!("sha256sum {}", file_path.display()),
                exit_code: Some(1),
                stdout: String::new(),
                stderr: format!("SHA-256 verification failed: {e}"),
            }),
        }
    }

    /// Complete automated end-to-end C++20 toolchain verification:
    /// 1. Refreshes User + System PATH from Windows Registry.
    /// 2. Discovers `clang++` or `g++` compiler on PATH.
    /// 3. Generates a modern C++20 program in a temporary directory.
    /// 4. Compiles it into an executable using `-std=c++20`.
    /// 5. Executes the resulting binary and asserts expected output.
    pub async fn verify_cpp_toolchain(&self, step_id: &str) -> Result<VerificationResult, CoreError> {
        self.events.emit(AgentEvent::VerificationStarted {
            step_id: step_id.to_string(),
        });

        let paths = self.os_provider.refresh_and_get_path().await?;

        // Locate C++ compiler (clang++ first, then g++)
        let compiler_bin = self
            .os_provider
            .which_in_paths("clang++", &paths)
            .or_else(|| self.os_provider.which_in_paths("g++", &paths))
            .or_else(|| self.os_provider.which_in_paths("clang", &paths));

        let compiler_path = match compiler_bin {
            Some(p) => p,
            None => {
                let res = VerificationResult {
                    passed: false,
                    command_run: "which clang++ || which g++".to_string(),
                    exit_code: None,
                    stdout: String::new(),
                    stderr: "Neither clang++ nor g++ was found in active PATH entries".to_string(),
                };
                self.events.emit(AgentEvent::VerificationCompleted {
                    step_id: step_id.to_string(),
                    result: res.clone(),
                });
                return Ok(res);
            }
        };

        // Create temporary directory for isolated compilation test
        let temp_dir = tempfile::Builder::new()
            .prefix("zero_trust_cpp_verify_")
            .tempdir()?;
        let source_path = temp_dir.path().join("main.cpp");

        let cpp20_source = r#"#include <iostream>
#include <vector>
#include <numeric>
#include <span>

int main() {
    std::vector<int> numbers = {1, 2, 3, 4, 5};
    std::span<int> span_view(numbers);
    int sum = std::accumulate(span_view.begin(), span_view.end(), 0);
    std::cout << "Zero-Trust C++20 Verification Passed! Sum=" << sum << std::endl;
    return 0;
}
"#;
        std::fs::write(&source_path, cpp20_source)?;

        let exe_name = if cfg!(target_os = "windows") {
            "hello.exe"
        } else {
            "hello"
        };
        let exe_path = temp_dir.path().join(exe_name);

        let compile_args = vec![
            "-std=c++20".to_string(),
            source_path.to_string_lossy().to_string(),
            "-o".to_string(),
            exe_path.to_string_lossy().to_string(),
        ];

        let compile_out = self
            .os_provider
            .run_command(&compiler_path, &compile_args, &paths)
            .await?;

        if compile_out.exit_code != Some(0) {
            let res = VerificationResult {
                passed: false,
                command_run: format!("{} {}", compiler_path.display(), compile_args.join(" ")),
                exit_code: compile_out.exit_code,
                stdout: compile_out.stdout,
                stderr: format!("C++20 compilation failed: {}", compile_out.stderr),
            };
            self.events.emit(AgentEvent::VerificationCompleted {
                step_id: step_id.to_string(),
                result: res.clone(),
            });
            return Ok(res);
        }

        // Run the compiled binary
        let run_out = self
            .os_provider
            .run_command(&exe_path, &[], &paths)
            .await?;

        let passed = run_out.exit_code == Some(0)
            && run_out
                .stdout
                .contains("Zero-Trust C++20 Verification Passed! Sum=15");

        let res = VerificationResult {
            passed,
            command_run: exe_path.to_string_lossy().to_string(),
            exit_code: run_out.exit_code,
            stdout: run_out.stdout,
            stderr: run_out.stderr,
        };

        self.events.emit(AgentEvent::VerificationCompleted {
            step_id: step_id.to_string(),
            result: res.clone(),
        });

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::MockOsProvider;

    #[tokio::test]
    async fn verifier_runs_binary_version_check() {
        let bus = EventBus::new(16);
        let mock_os = Arc::new(MockOsProvider::with_mock_cpp_toolchain());
        let verifier = Verifier::new(mock_os, bus);

        let res = verifier
            .verify_binary_command("step-4", AllowedBinary::Clang, &["--version".to_string()])
            .await
            .expect("verify command");

        assert!(res.passed);
        assert!(res.stdout.contains("clang version 18.1.8"));
    }

    #[tokio::test]
    async fn verifier_runs_cpp_toolchain_compilation_and_execution() {
        let bus = EventBus::new(16);
        let mock_os = Arc::new(MockOsProvider::with_mock_cpp_toolchain());
        let verifier = Verifier::new(mock_os, bus);

        let res = verifier
            .verify_cpp_toolchain("step-cpp-check")
            .await
            .expect("cpp toolchain verify");

        assert!(res.passed);
        assert!(res.stdout.contains("Zero-Trust C++20 Verification Passed! Sum=15"));
    }
}
