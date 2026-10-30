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

pub const DEFAULT_CRATES_IO_BASE_URL: &str = "https://crates.io/api/v1";

/// Adapter for Rust binaries published on crates.io, installed with `cargo install --locked`.
pub struct CargoAdapter {
    http: reqwest::Client,
    os_provider: Arc<dyn OsProvider>,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    krate: CrateInfo,
    #[serde(default)]
    versions: Vec<CrateVersion>,
}

#[derive(Debug, Deserialize)]
struct CrateInfo {
    name: String,
    #[serde(default)]
    max_stable_version: Option<String>,
    #[serde(default)]
    newest_version: Option<String>,
    #[serde(default)]
    downloads: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CrateVersion {
    num: String,
    #[serde(default)]
    checksum: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    yanked: bool,
}

/// True when `cargo install --list` output shows `name` as installed.
pub fn cargo_list_has(stdout: &str, name: &str) -> bool {
    let prefix = format!("{name} v");
    stdout.lines().any(|l| l.starts_with(&prefix))
}

impl CargoAdapter {
    pub fn new(os_provider: Arc<dyn OsProvider>) -> Self {
        Self {
            http: registry_http_client(30),
            os_provider,
            base_url: DEFAULT_CRATES_IO_BASE_URL.to_string(),
        }
    }

    async fn locate_cargo(&self) -> Result<(PathBuf, Vec<PathBuf>), AdapterError> {
        let mut paths = self
            .os_provider
            .refresh_and_get_path()
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;
        if let Some(home) = crate::paths::user_profile_dir() {
            let cargo_bin = home.join(".cargo").join("bin");
            if !paths.contains(&cargo_bin) {
                paths.push(cargo_bin);
            }
        }
        let bin = self
            .os_provider
            .which_in_paths("cargo", &paths)
            .ok_or_else(|| AdapterError::Other("cargo executable not found; install Rust first".to_string()))?;
        Ok((bin, paths))
    }
}

#[async_trait]
impl SourceAdapter for CargoAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::Cargo
    }

    async fn resolve_metadata(
        &self,
        package_id: &str,
        version: Option<&str>,
    ) -> Result<PackageMetadata, AdapterError> {
        let url = format!("{}/crates/{}", self.base_url, package_id);
        let response = self.http.get(&url).send().await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AdapterError::PackageNotFound {
                adapter: AdapterKind::Cargo,
                package_id: package_id.to_string(),
            });
        }
        let body: CrateResponse = response.error_for_status()?.json().await?;

        let wanted = match version {
            Some(v) if !v.is_empty() && v != "latest" => v.to_string(),
            _ => body
                .krate
                .max_stable_version
                .clone()
                .or_else(|| body.krate.newest_version.clone())
                .ok_or_else(|| AdapterError::Other(format!("crate {package_id} has no published version")))?,
        };
        let ver = body
            .versions
            .iter()
            .find(|v| v.num == wanted && !v.yanked)
            .ok_or_else(|| AdapterError::PackageNotFound {
                adapter: AdapterKind::Cargo,
                package_id: format!("{package_id}@{wanted}"),
            })?;

        let name = body.krate.name.clone();
        let mut meta = PackageMetadata::bare(AdapterKind::Cargo, name.clone(), ver.num.clone());
        meta.published_at = parse_rfc3339(ver.created_at.as_deref());
        meta.age_days = age_days_since(ver.created_at.as_deref());
        meta.download_count = body.krate.downloads;
        meta.download_url = Some(format!(
            "https://static.crates.io/crates/{name}/{name}-{}.crate",
            ver.num
        ));
        meta.expected_sha256 = ver.checksum.clone().map(|c| c.to_ascii_lowercase());
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
        let mut args = vec!["install".to_string(), metadata.package_id.clone()];
        if !metadata.resolved_version.is_empty() && metadata.resolved_version != "latest" {
            args.push("--version".to_string());
            args.push(metadata.resolved_version.clone());
        }
        args.push("--locked".to_string());
        let rollback_hint = Some(format!("cargo uninstall {}", metadata.package_id));

        if ctx.dry_run {
            return Ok(ExecutionReceipt::new(
                AdapterKind::Cargo,
                metadata.package_id.clone(),
                metadata.resolved_version.clone(),
                true,
                vec![format!("[Dry Run] Would execute: cargo {}", args.join(" "))],
                rollback_hint,
            ));
        }

        let (cargo_bin, paths) = self.locate_cargo().await?;
        let out = self
            .os_provider
            .run_command_cancellable(&cargo_bin, &args, &paths, None, Some(&ctx.cancel))
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;
        if out.exit_code != Some(0) {
            return Err(AdapterError::CommandFailed {
                exit_code: out.exit_code,
                stdout: out.stdout,
                stderr: out.stderr,
            });
        }

        Ok(ExecutionReceipt::new(
            AdapterKind::Cargo,
            metadata.package_id.clone(),
            metadata.resolved_version.clone(),
            false,
            vec![format!(
                "Installed {} ({}) via cargo install",
                metadata.package_id, metadata.resolved_version
            )],
            rollback_hint,
        )
        .with_uninstall_argv(vec![
            cargo_bin.display().to_string(),
            "uninstall".to_string(),
            metadata.package_id.clone(),
        ]))
    }

    async fn verify(
        &self,
        metadata: &PackageMetadata,
        _ctx: &ExecutionContext,
    ) -> Result<VerificationResult, AdapterError> {
        let command_run = "cargo install --list".to_string();
        let (cargo_bin, paths) = match self.locate_cargo().await {
            Ok(found) => found,
            Err(e) => return Ok(VerificationResult::failed(command_run, e.to_string())),
        };
        let out = self
            .os_provider
            .run_command(&cargo_bin, &["install".to_string(), "--list".to_string()], &paths)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;
        let passed = out.exit_code == Some(0) && cargo_list_has(&out.stdout, &metadata.package_id);
        Ok(VerificationResult {
            passed,
            command_run,
            exit_code: out.exit_code,
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_list_parsing() {
        let out = "ripgrep v14.1.0:\n    rg.exe\ncargo-edit v0.12.0:\n    cargo-add.exe\n";
        assert!(cargo_list_has(out, "ripgrep"));
        assert!(!cargo_list_has(out, "rip"));
        assert!(!cargo_list_has(out, "bat"));
    }
}
