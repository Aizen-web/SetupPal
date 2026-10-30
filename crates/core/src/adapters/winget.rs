use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use chrono::{NaiveDate, TimeZone, Utc};

use crate::adapters::{
    AdapterError, ExecutionContext, ExecutionReceipt, FetchedArtifact, PackageMetadata,
    SourceAdapter, VerificationResult,
};
use crate::os::OsProvider;
use crate::types::AdapterKind;

/// Fields read from `winget show --id <id> --exact` output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WingetShowInfo {
    pub name: Option<String>,
    pub version: Option<String>,
    /// Lowercase first word of `Installer Type` (`portable (zip)` becomes `portable`).
    pub installer_type: Option<String>,
    /// Lowercase `Installer Scope` (`user` or `machine`) when the manifest declares one.
    pub installer_scope: Option<String>,
    pub installer_url: Option<String>,
    pub installer_sha256: Option<String>,
    pub release_date: Option<NaiveDate>,
}

/// Parses English `winget show` output. Unknown or missing lines leave fields as `None`.
pub fn parse_winget_show(stdout: &str) -> WingetShowInfo {
    let mut info = WingetShowInfo::default();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Found ") {
            if let Some(idx) = rest.rfind(" [") {
                info.name = Some(rest[..idx].trim().to_string());
            }
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim().to_ascii_lowercase().as_str() {
            "version" if info.version.is_none() => info.version = Some(value.to_string()),
            "installer type" => {
                info.installer_type = value
                    .split_whitespace()
                    .next()
                    .map(|t| t.to_ascii_lowercase());
            }
            "installer scope" => info.installer_scope = Some(value.to_ascii_lowercase()),
            "installer url" => info.installer_url = Some(value.to_string()),
            "installer sha256" => {
                let digest = value.to_ascii_lowercase();
                if crate::safety::policy::is_valid_sha256_hex(&digest) {
                    info.installer_sha256 = Some(digest);
                }
            }
            "release date" => info.release_date = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok(),
            _ => {}
        }
    }
    info
}

/// Builds the argument list for `winget show`.
pub fn winget_show_args(package_id: &str, version: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "show".to_string(),
        "--id".to_string(),
        package_id.to_string(),
        "--exact".to_string(),
        "--accept-source-agreements".to_string(),
        "--disable-interactivity".to_string(),
    ];
    if let Some(v) = version.filter(|v| !v.is_empty() && *v != "latest") {
        args.push("--version".to_string());
        args.push(v.to_string());
    }
    args
}

/// Native adapter for Windows Package Manager (`winget`).
pub struct WingetAdapter {
    os_provider: Arc<dyn OsProvider>,
}

impl WingetAdapter {
    pub fn new(os_provider: Arc<dyn OsProvider>) -> Self {
        Self { os_provider }
    }

    async fn locate_winget(&self) -> Result<(PathBuf, Vec<PathBuf>), AdapterError> {
        let paths = self
            .os_provider
            .refresh_and_get_path()
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;
        let bin = self
            .os_provider
            .which_in_paths("winget", &paths)
            .ok_or_else(|| AdapterError::Other("winget executable not found on host".to_string()))?;
        Ok((bin, paths))
    }

    fn uninstall_argv(package_id: &str) -> Vec<String> {
        vec![
            "winget".to_string(),
            "uninstall".to_string(),
            "--id".to_string(),
            package_id.to_string(),
            "--exact".to_string(),
            "--silent".to_string(),
            "--accept-source-agreements".to_string(),
            "--disable-interactivity".to_string(),
        ]
    }

    fn list_args(package_id: &str) -> Vec<String> {
        vec![
            "list".to_string(),
            "--id".to_string(),
            package_id.to_string(),
            "--exact".to_string(),
            "--accept-source-agreements".to_string(),
            "--disable-interactivity".to_string(),
        ]
    }

    fn list_output_has_id(stdout: &str, package_id: &str) -> bool {
        stdout
            .to_ascii_lowercase()
            .contains(&package_id.to_ascii_lowercase())
    }

    /// `--scope user` unless the manifest only offers a machine-wide installer.
    fn install_args(metadata: &PackageMetadata) -> Vec<String> {
        let mut args = vec![
            "install".to_string(),
            "--id".to_string(),
            metadata.package_id.clone(),
            "--exact".to_string(),
        ];
        if metadata.installer_scope.as_deref() != Some("machine") {
            args.push("--scope".to_string());
            args.push("user".to_string());
        }
        args.extend(
            [
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
                "--disable-interactivity",
            ]
            .map(String::from),
        );
        if !metadata.resolved_version.is_empty() && metadata.resolved_version != "latest" {
            args.push("--version".to_string());
            args.push(metadata.resolved_version.clone());
        }
        args
    }
}

#[async_trait]
impl SourceAdapter for WingetAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::Winget
    }

    async fn resolve_metadata(
        &self,
        package_id: &str,
        version: Option<&str>,
    ) -> Result<PackageMetadata, AdapterError> {
        let (winget_bin, paths) = self.locate_winget().await?;
        let out = self
            .os_provider
            .run_command(&winget_bin, &winget_show_args(package_id, version), &paths)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if out.exit_code != Some(0) {
            if out.stdout.to_ascii_lowercase().contains("no package found") {
                return Err(AdapterError::PackageNotFound {
                    adapter: AdapterKind::Winget,
                    package_id: package_id.to_string(),
                });
            }
            return Err(AdapterError::CommandFailed {
                exit_code: out.exit_code,
                stdout: out.stdout,
                stderr: out.stderr,
            });
        }

        let info = parse_winget_show(&out.stdout);
        let published_at = info
            .release_date
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| Utc.from_utc_datetime(&dt));
        let age_days = published_at.map(|p| Utc::now().signed_duration_since(p).num_days().max(0) as u64);

        let mut meta = PackageMetadata::bare(
            AdapterKind::Winget,
            package_id,
            info.version
                .clone()
                .or_else(|| version.map(str::to_string))
                .unwrap_or_else(|| "latest".to_string()),
        );
        meta.published_at = published_at;
        meta.age_days = age_days;
        meta.expected_sha256 = info.installer_sha256;
        meta.installer_type = info.installer_type;
        meta.installer_scope = info.installer_scope;
        Ok(meta)
    }

    async fn fetch_artifact(
        &self,
        metadata: &PackageMetadata,
        _destination: &Path,
        _ctx: &ExecutionContext,
    ) -> Result<FetchedArtifact, AdapterError> {
        Err(AdapterError::Other(format!(
            "winget downloads and hash-checks {} itself; use an install_package step instead",
            metadata.package_id
        )))
    }

    async fn install(
        &self,
        metadata: &PackageMetadata,
        _artifact: Option<&FetchedArtifact>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, AdapterError> {
        let args = Self::install_args(metadata);
        let rollback_hint = Some(format!("winget uninstall --id {} --exact", metadata.package_id));

        if ctx.dry_run {
            return Ok(ExecutionReceipt::new(
                AdapterKind::Winget,
                metadata.package_id.clone(),
                metadata.resolved_version.clone(),
                true,
                vec![format!("[Dry Run] Would execute: winget {}", args.join(" "))],
                rollback_hint,
            ));
        }

        let (winget_bin, paths) = self.locate_winget().await?;

        if let Ok(list_out) = self
            .os_provider
            .run_command(&winget_bin, &Self::list_args(&metadata.package_id), &paths)
            .await
        {
            if list_out.exit_code == Some(0) && Self::list_output_has_id(&list_out.stdout, &metadata.package_id) {
                return Ok(ExecutionReceipt::new(
                    AdapterKind::Winget,
                    metadata.package_id.clone(),
                    metadata.resolved_version.clone(),
                    false,
                    vec![format!("Package {} is already installed on this system.", metadata.package_id)],
                    None,
                ));
            }
        }

        let out = self
            .os_provider
            .run_command_cancellable(&winget_bin, &args, &paths, None, Some(&ctx.cancel))
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
            AdapterKind::Winget,
            metadata.package_id.clone(),
            metadata.resolved_version.clone(),
            false,
            vec![format!(
                "Installed {} ({}) via winget",
                metadata.package_id, metadata.resolved_version
            )],
            rollback_hint,
        )
        .with_uninstall_argv(Self::uninstall_argv(&metadata.package_id)))
    }

    async fn verify(
        &self,
        metadata: &PackageMetadata,
        _ctx: &ExecutionContext,
    ) -> Result<VerificationResult, AdapterError> {
        let command_run = format!("winget list --id {} --exact", metadata.package_id);
        let (winget_bin, paths) = match self.locate_winget().await {
            Ok(found) => found,
            Err(e) => return Ok(VerificationResult::failed(command_run, e.to_string())),
        };

        let out = self
            .os_provider
            .run_command(&winget_bin, &Self::list_args(&metadata.package_id), &paths)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        let passed = out.exit_code == Some(0) && Self::list_output_has_id(&out.stdout, &metadata.package_id);
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
    use crate::os::mock::{fail, ok, MockOsProvider};

    const GIT_SHOW: &str = "Found Git [Git.Git]\nVersion: 2.56.0.3\nPublisher: The Git Development Community\nInstaller:\n  Installer Type: inno\n  Installer Url: https://github.com/git-for-windows/git/releases/download/v2.56.0.windows.3/Git-2.56.0.3-64-bit.exe\n  Installer SHA256: dc6d3db061999136e139f31376c5582a9ed6f36a4ea12f15e54ad7fa2bfc0bf2\n  Release Date: 2026-01-02\n";

    #[test]
    fn parses_winget_show_fields() {
        let info = parse_winget_show(GIT_SHOW);
        assert_eq!(info.name.as_deref(), Some("Git"));
        assert_eq!(info.version.as_deref(), Some("2.56.0.3"));
        assert_eq!(info.installer_type.as_deref(), Some("inno"));
        assert_eq!(info.installer_scope, None);
        assert_eq!(
            info.installer_sha256.as_deref(),
            Some("dc6d3db061999136e139f31376c5582a9ed6f36a4ea12f15e54ad7fa2bfc0bf2")
        );
        assert!(info.release_date.is_some());

        let portable = parse_winget_show("Installer:\n  Installer Type: portable (zip)\n  Installer Scope: Machine\n");
        assert_eq!(portable.installer_type.as_deref(), Some("portable"));
        assert_eq!(portable.installer_scope.as_deref(), Some("machine"));
    }

    #[tokio::test]
    async fn resolve_metadata_uses_manifest_digest() {
        let os = MockOsProvider::new()
            .with_binary("winget", "C:/mock/winget.exe")
            .on("winget", &["show"], ok(GIT_SHOW));
        let adapter = WingetAdapter::new(Arc::new(os));
        let meta = adapter.resolve_metadata("Git.Git", None).await.unwrap();
        assert_eq!(meta.resolved_version, "2.56.0.3");
        assert!(meta.expected_sha256.is_some());
        assert_eq!(meta.download_count, None);
    }

    #[tokio::test]
    async fn resolve_metadata_without_winget_is_an_error() {
        let adapter = WingetAdapter::new(Arc::new(MockOsProvider::new()));
        assert!(adapter.resolve_metadata("Git.Git", None).await.is_err());
    }

    #[tokio::test]
    async fn verify_fails_when_package_is_not_listed() {
        let os = MockOsProvider::new()
            .with_binary("winget", "C:/mock/winget.exe")
            .on("winget", &["list"], fail(1, "No installed package found matching input criteria."));
        let adapter = WingetAdapter::new(Arc::new(os));
        let meta = PackageMetadata::bare(AdapterKind::Winget, "Git.Git", "latest");
        let ctx = ExecutionContext::new(false, PathBuf::from("."), None,
        );
        assert!(!adapter.verify(&meta, &ctx).await.unwrap().passed);
    }

    #[test]
    fn machine_scope_drops_user_scope_flag() {
        let mut meta = PackageMetadata::bare(AdapterKind::Winget, "LLVM.LLVM", "latest");
        assert!(WingetAdapter::install_args(&meta).contains(&"user".to_string()));
        meta.installer_scope = Some("machine".to_string());
        assert!(!WingetAdapter::install_args(&meta).contains(&"--scope".to_string()));
    }
}
