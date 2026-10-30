use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use serde::Deserialize;

use crate::adapters::{
    age_days_since, fetch_with_digests, parse_rfc3339, registry_http_client, AdapterError,
    ExecutionContext, ExecutionReceipt, FetchedArtifact, PackageMetadata, SourceAdapter,
    VerificationResult,
};
use crate::os::OsProvider;
use crate::types::AdapterKind;

pub const DEFAULT_PYPI_BASE_URL: &str = "https://pypi.org";

/// Native adapter for the Python Package Index (PyPI).
/// When the execution context carries an isolated venv, every pip call uses that venv's
/// interpreter and never falls back to the global Python.
pub struct PyPIAdapter {
    http: reqwest::Client,
    os_provider: Arc<dyn OsProvider>,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct PyPiResponse {
    info: PyPiInfo,
    #[serde(default)]
    urls: Vec<PyPiFile>,
    /// All releases; used for first-publish age (R13).
    #[serde(default)]
    releases: std::collections::HashMap<String, Vec<PyPiFile>>,
}

#[derive(Debug, Deserialize)]
struct PyPiInfo {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize, Clone)]
struct PyPiFile {
    filename: String,
    url: String,
    #[serde(default)]
    packagetype: String,
    #[serde(default)]
    upload_time_iso_8601: Option<String>,
    #[serde(default)]
    digests: PyPiDigests,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct PyPiDigests {
    #[serde(default)]
    sha256: Option<String>,
}

/// Interpreter inside a venv (`Scripts\python.exe` on Windows, `bin/python` elsewhere).
pub fn venv_python(env_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        env_dir.join("Scripts").join("python.exe")
    } else {
        env_dir.join("bin").join("python")
    }
}

impl PyPIAdapter {
    pub fn new(os_provider: Arc<dyn OsProvider>) -> Self {
        Self {
            http: registry_http_client(30),
            os_provider,
            base_url: DEFAULT_PYPI_BASE_URL.to_string(),
        }
    }

    pub fn with_base_url(
        http: reqwest::Client,
        os_provider: Arc<dyn OsProvider>,
        base_url: String,
    ) -> Self {
        Self {
            http,
            os_provider,
            base_url,
        }
    }

    fn select_best_file(files: &[PyPiFile]) -> Option<PyPiFile> {
        // Preference: Windows wheel -> any-platform wheel -> any wheel -> first available
        files
            .iter()
            .find(|f| {
                f.packagetype == "bdist_wheel"
                    && (f.filename.contains("win_amd64") || f.filename.contains("win32"))
            })
            .or_else(|| {
                files
                    .iter()
                    .find(|f| f.packagetype == "bdist_wheel" && f.filename.contains("none-any"))
            })
            .or_else(|| files.iter().find(|f| f.packagetype == "bdist_wheel"))
            .or_else(|| files.first())
            .cloned()
    }

    /// Resolves the interpreter for pip. With an isolated env the venv interpreter is
    /// mandatory; otherwise the first `python` / `python3` on PATH is used.
    fn find_python_binary(
        ctx: &ExecutionContext,
        os_provider: &Arc<dyn OsProvider>,
        paths: &[PathBuf],
    ) -> Result<PathBuf, AdapterError> {
        if let Some(env_dir) = &ctx.isolated_env {
            let bin = venv_python(env_dir);
            return if bin.is_file() {
                Ok(bin)
            } else {
                Err(AdapterError::Other(format!(
                    "isolated environment interpreter {} is missing",
                    bin.display()
                )))
            };
        }
        os_provider
            .which_in_paths("python", paths)
            .or_else(|| os_provider.which_in_paths("python3", paths))
            .ok_or_else(|| AdapterError::Other("Python executable not found".to_string()))
    }
}

#[async_trait]
impl SourceAdapter for PyPIAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::PyPI
    }

    async fn resolve_metadata(
        &self,
        package_id: &str,
        version: Option<&str>,
    ) -> Result<PackageMetadata, AdapterError> {
        let url = match version {
            Some(ver) if !ver.is_empty() && ver != "latest" => {
                format!("{}/pypi/{}/{}/json", self.base_url, package_id, ver)
            }
            _ => format!("{}/pypi/{}/json", self.base_url, package_id),
        };

        let response = self.http.get(&url).send().await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AdapterError::PackageNotFound {
                adapter: AdapterKind::PyPI,
                package_id: package_id.to_string(),
            });
        }
        let body: PyPiResponse = response.error_for_status()?.json().await?;

        let best_file = Self::select_best_file(&body.urls);
        let uploaded = best_file.as_ref().and_then(|f| f.upload_time_iso_8601.clone());
        let first_publish = body
            .releases
            .values()
            .flatten()
            .filter_map(|f| f.upload_time_iso_8601.as_deref())
            .min()
            .map(str::to_string);

        let mut meta = PackageMetadata::bare(AdapterKind::PyPI, body.info.name, body.info.version);
        meta.published_at = parse_rfc3339(uploaded.as_deref());
        meta.age_days = age_days_since(first_publish.as_deref())
            .or_else(|| age_days_since(uploaded.as_deref()));
        meta.download_url = best_file.as_ref().map(|f| f.url.clone());
        meta.expected_sha256 = best_file
            .and_then(|f| f.digests.sha256)
            .map(|d| d.to_ascii_lowercase());
        Ok(meta)
    }

    async fn fetch_artifact(
        &self,
        metadata: &PackageMetadata,
        destination: &Path,
        ctx: &ExecutionContext,
    ) -> Result<FetchedArtifact, AdapterError> {
        fetch_with_digests(&self.http, metadata, destination, ctx).await
    }

    async fn install(
        &self,
        metadata: &PackageMetadata,
        _artifact: Option<&FetchedArtifact>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, AdapterError> {
        let pkg_spec = if metadata.resolved_version.is_empty() || metadata.resolved_version == "latest" {
            metadata.package_id.clone()
        } else {
            format!("{}=={}", metadata.package_id, metadata.resolved_version)
        };
        let target = match &ctx.isolated_env {
            Some(env) => format!("venv {}", env.display()),
            None => "the global Python".to_string(),
        };

        if ctx.dry_run {
            return Ok(ExecutionReceipt::new(
                AdapterKind::PyPI,
                metadata.package_id.clone(),
                metadata.resolved_version.clone(),
                true,
                vec![format!("[Dry Run] Would execute: python -m pip install {pkg_spec} into {target}")],
                Some(format!("python -m pip uninstall -y {}", metadata.package_id)),
            ));
        }

        let paths = self
            .os_provider
            .refresh_and_get_path()
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;
        let python_bin = Self::find_python_binary(ctx, &self.os_provider, &paths)?;

        let args = vec![
            "-m".to_string(),
            "pip".to_string(),
            "install".to_string(),
            pkg_spec,
            "--no-warn-script-location".to_string(),
            "--disable-pip-version-check".to_string(),
        ];
        let out = self
            .os_provider
            .run_command_cancellable(&python_bin, &args, &paths, None, Some(&ctx.cancel))
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if out.exit_code != Some(0) {
            return Err(AdapterError::CommandFailed {
                exit_code: out.exit_code,
                stdout: out.stdout,
                stderr: out.stderr,
            });
        }

        let python = python_bin.display().to_string();
        Ok(ExecutionReceipt::new(
            AdapterKind::PyPI,
            metadata.package_id.clone(),
            metadata.resolved_version.clone(),
            false,
            vec![format!(
                "Installed {} ({}) into {target} via pip",
                metadata.package_id, metadata.resolved_version
            )],
            Some(format!("{python} -m pip uninstall -y {}", metadata.package_id)),
        )
        .with_uninstall_argv(vec![
            python,
            "-m".to_string(),
            "pip".to_string(),
            "uninstall".to_string(),
            "-y".to_string(),
            metadata.package_id.clone(),
        ]))
    }

    async fn verify(
        &self,
        metadata: &PackageMetadata,
        ctx: &ExecutionContext,
    ) -> Result<VerificationResult, AdapterError> {
        let paths = self
            .os_provider
            .refresh_and_get_path()
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        let python_bin = match Self::find_python_binary(ctx, &self.os_provider, &paths) {
            Ok(b) => b,
            Err(e) => {
                return Ok(VerificationResult::failed(
                    format!("python -m pip show {}", metadata.package_id),
                    e.to_string(),
                ))
            }
        };

        let args = vec![
            "-m".to_string(),
            "pip".to_string(),
            "show".to_string(),
            metadata.package_id.clone(),
        ];
        let out = self
            .os_provider
            .run_command(&python_bin, &args, &paths)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        Ok(VerificationResult {
            passed: out.exit_code == Some(0),
            command_run: format!("{} -m pip show {}", python_bin.display(), metadata.package_id),
            exit_code: out.exit_code,
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::mock::{ok, MockOsProvider};

    #[tokio::test]
    async fn install_uses_venv_python_and_never_global() {
        let env = tempfile::tempdir().unwrap();
        let py = venv_python(env.path());
        std::fs::create_dir_all(py.parent().unwrap()).unwrap();
        std::fs::write(&py, b"").unwrap();

        let os = Arc::new(
            MockOsProvider::new()
                .with_binary("python", "C:/global/python.exe")
                .on_fn(|_, args, _| (args.get(2).map(String::as_str) == Some("install")).then(|| ok("ok"))),
        );
        let adapter = PyPIAdapter::new(os.clone());
        let meta = PackageMetadata::bare(AdapterKind::PyPI, "numpy", "2.1.1");
        let ctx = ExecutionContext::new(false, PathBuf::from("."), Some(env.path().to_path_buf()),
        );
        adapter.install(&meta, None, &ctx).await.unwrap();
        assert_eq!(os.calls()[0].binary, py);
    }

    #[tokio::test]
    async fn missing_venv_interpreter_fails_instead_of_using_global() {
        let env = tempfile::tempdir().unwrap();
        let os = Arc::new(MockOsProvider::new().with_binary("python", "C:/global/python.exe"));
        let adapter = PyPIAdapter::new(os.clone());
        let meta = PackageMetadata::bare(AdapterKind::PyPI, "numpy", "2.1.1");
        let ctx = ExecutionContext::new(false, PathBuf::from("."), Some(env.path().to_path_buf()),
        );
        assert!(adapter.install(&meta, None, &ctx).await.is_err());
        assert!(os.calls().is_empty());
    }
}
