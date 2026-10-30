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
    /// How many versions `winget show` listed (identifier age proxy when > 1).
    pub version_count: Option<usize>,
}

/// Parses English `winget show` output. Unknown or missing lines leave fields as `None`.
pub fn parse_winget_show(stdout: &str) -> WingetShowInfo {
    let mut info = WingetShowInfo::default();
    let mut in_versions = false;
    let mut listed_versions = 0usize;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Found ") {
            if let Some(idx) = rest.rfind(" [") {
                info.name = Some(rest[..idx].trim().to_string());
            }
            in_versions = false;
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("available versions") || lower == "versions" {
            in_versions = true;
            // "Available Versions: 1.0, 1.1" on one line
            if let Some((_, rest)) = trimmed.split_once(':') {
                let count = rest
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|p| !p.is_empty())
                    .count();
                if count > 0 {
                    listed_versions = listed_versions.max(count);
                }
            }
            continue;
        }
        if in_versions {
            if trimmed.is_empty() || trimmed.contains(':') && !trimmed.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c.is_whitespace()) {
                // Left the versions block when a new Key: value section starts (except pure version lines).
                if trimmed.contains(':') {
                    in_versions = false;
                } else if !trimmed.is_empty() {
                    listed_versions += 1;
                    continue;
                }
            } else if !trimmed.is_empty() {
                listed_versions += 1;
                continue;
            }
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
    if listed_versions > 0 {
        info.version_count = Some(listed_versions);
    }
    info
}

/// Package-identifier age for winget: established when more than one version exists.
/// Otherwise uses the latest release date as a stand-in for first-publish.
pub fn winget_identifier_age_days(info: &WingetShowInfo) -> Option<u64> {
    if info.version_count.unwrap_or(1) > 1 {
        // Multiple published versions → treat as established (not "newly published").
        return None;
    }
    let published_at = info.release_date.and_then(|d| d.and_hms_opt(0, 0, 0))?;
    let published_at = Utc.from_utc_datetime(&published_at);
    Some(Utc::now().signed_duration_since(published_at).num_days().max(0) as u64)
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

    /// Pass `--scope user` only when the manifest lists a user-scope installer.
    /// Manifests with no scope (LMMS and many others) must not get a scope flag.
    fn install_args(metadata: &PackageMetadata) -> Vec<String> {
        Self::install_args_with_scope(metadata, metadata.installer_scope.as_deref() == Some("user"))
    }

    fn install_args_with_scope(metadata: &PackageMetadata, force_user_scope: bool) -> Vec<String> {
        let mut args = vec![
            "install".to_string(),
            "--id".to_string(),
            metadata.package_id.clone(),
            "--exact".to_string(),
        ];
        if force_user_scope {
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

/// Maps common winget HRESULTs to a plain sentence. Includes the last non-empty output line.
pub fn format_winget_failure(exit_code: Option<i32>, stdout: &str, stderr: &str) -> String {
    let code = exit_code.unwrap_or(0);
    // winget returns these as signed i32 (e.g. -1978335216 == 0x8A150010).
    let reason = match code as u32 {
        0x8A150010 => "No applicable installer found for this PC (wrong architecture or installer scope).",
        0x8A150011 => "No package found matching that id.",
        0x8A15002B => "Another install is already in progress. Wait for it to finish, then try again.",
        0x8A150014 => "The installer hash did not match. SetupPal will not run an unverified installer.",
        0x8A150063 => "This package requires administrator approval.",
        _ if code != 0 => "winget reported a failure.",
        _ => "winget did not complete successfully.",
    };
    let detail = [stderr, stdout]
        .iter()
        .flat_map(|s| s.lines().rev())
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if detail.is_empty() {
        format!("{reason} (exit {exit_code:?})")
    } else {
        format!("{reason} {detail} (exit {exit_code:?})")
    }
}

/// True when winget failed because no installer matched the requested scope.
pub fn is_no_applicable_installer(exit_code: Option<i32>) -> bool {
    exit_code.map(|c| c as u32) == Some(0x8A150010)
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
        let age_days = winget_identifier_age_days(&info);

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

        #[cfg(windows)]
        if let Some(msg) = crate::os::windows::blocking_installer_message().await {
            return Err(AdapterError::Other(msg));
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

        let user_path_before = self
            .os_provider
            .read_user_path_raw()
            .await
            .unwrap_or_default();

        let out = self
            .os_provider
            .run_command_with_options(
                &winget_bin,
                &args,
                &paths,
                None,
                Some(&ctx.cancel),
                ctx.on_progress.clone(),
            )
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        // Scope was forced from the manifest; if winget still says no installer, retry once
        // without `--scope` (common for packages that list user scope but ship machine-only builds).
        let out = if out.exit_code != Some(0)
            && is_no_applicable_installer(out.exit_code)
            && args.iter().any(|a| a == "--scope")
        {
            let retry_args = Self::install_args_with_scope(metadata, false);
            self.os_provider
                .run_command_with_options(
                    &winget_bin,
                    &retry_args,
                    &paths,
                    None,
                    Some(&ctx.cancel),
                    ctx.on_progress.clone(),
                )
                .await
                .map_err(|e| AdapterError::Other(e.to_string()))?
        } else {
            out
        };

        if out.exit_code != Some(0) {
            return Err(AdapterError::CommandFailed {
                exit_code: out.exit_code,
                stdout: out.stdout.clone(),
                stderr: format_winget_failure(out.exit_code, &out.stdout, &out.stderr),
            });
        }

        // Refresh PATH so the post-install diff sees what the installer registered.
        let _ = self.os_provider.refresh_and_get_path().await;
        let user_path_after = self
            .os_provider
            .read_user_path_raw()
            .await
            .unwrap_or_default();
        let path_entries_added =
            crate::cleanup::path_entries_added(&user_path_before, &user_path_after);
        let install_location =
            crate::cleanup::infer_install_location(&metadata.package_id, &path_entries_added);

        let mut receipt = ExecutionReceipt::new(
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
        .with_uninstall_argv(Self::uninstall_argv(&metadata.package_id));
        receipt.user_path_before = Some(user_path_before);
        receipt.path_entries_added = path_entries_added;
        receipt.install_location = install_location;
        Ok(receipt)
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

    #[test]
    fn established_packages_have_no_identifier_age() {
        let multi = parse_winget_show(
            "Found OBS [OBSProject.OBSStudio]\nVersion: 31.0.0\nRelease Date: 2026-10-20\nAvailable Versions:\n  31.0.0\n  30.2.3\n  30.1.0\n",
        );
        assert!(multi.version_count.unwrap_or(0) > 1);
        assert_eq!(winget_identifier_age_days(&multi), None);

        let brand_new = parse_winget_show(
            "Found NewApp [Publisher.NewApp]\nVersion: 0.1.0\nRelease Date: 2026-10-20\n",
        );
        let age = winget_identifier_age_days(&brand_new).expect("single-version uses release date");
        assert!(age < 30, "expected young identifier, got {age}");
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
    fn scope_flag_only_when_manifest_lists_user() {
        let mut meta = PackageMetadata::bare(AdapterKind::Winget, "LMMS.LMMS", "latest");
        assert!(
            !WingetAdapter::install_args(&meta).contains(&"--scope".to_string()),
            "no scope in manifest → no --scope flag"
        );
        meta.installer_scope = Some("user".to_string());
        assert!(WingetAdapter::install_args(&meta).contains(&"user".to_string()));
        meta.installer_scope = Some("machine".to_string());
        assert!(!WingetAdapter::install_args(&meta).contains(&"--scope".to_string()));
    }

    #[test]
    fn winget_hresult_maps_to_plain_words() {
        let msg = format_winget_failure(Some(-1978335216), "", "No applicable installer found.");
        assert!(msg.contains("No applicable installer"));
        assert!(!msg.trim_end_matches(|c: char| c.is_ascii_digit() || c == '-' || c == ':' || c == ' ' || c == '(' || c == ')').is_empty());
    }
}
