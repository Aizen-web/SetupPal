pub mod cargo;
pub mod github;
pub mod modrinth;
pub mod npm;
pub mod pypi;
pub mod vendor;
pub mod winget;

pub use cargo::CargoAdapter;
pub use github::GitHubReleasesAdapter;
pub use modrinth::ModrinthAdapter;
pub use npm::NpmAdapter;
pub use pypi::PyPIAdapter;
pub use vendor::VendorUrlAdapter;
pub use winget::WingetAdapter;

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::safety::hash::{compute_file_sha256, compute_file_sha512};
use crate::safety::policy::is_url_domain_allowlisted;
use crate::types::AdapterKind;

/// Upper bound for a single artifact download (2 GiB).
pub const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageMetadata {
    pub adapter: AdapterKind,
    pub package_id: String,
    pub resolved_version: String,
    pub published_at: Option<DateTime<Utc>>,
    pub age_days: Option<u64>,
    pub download_count: Option<u64>,
    pub download_url: Option<String>,
    /// Digest reported by the upstream registry API (never a placeholder).
    pub expected_sha256: Option<String>,
    pub expected_sha512: Option<String>,
    pub install_script_snippet: Option<String>,
    /// Installer technology reported upstream (for example winget `Installer Type`).
    #[serde(default)]
    pub installer_type: Option<String>,
    /// Installer scope reported upstream (`user` or `machine`).
    #[serde(default)]
    pub installer_scope: Option<String>,
}

impl PackageMetadata {
    /// Metadata with only identity fields set; everything else is unknown.
    pub fn bare(adapter: AdapterKind, package_id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            adapter,
            package_id: package_id.into(),
            resolved_version: version.into(),
            published_at: None,
            age_days: None,
            download_count: None,
            download_url: None,
            expected_sha256: None,
            expected_sha512: None,
            install_script_snippet: None,
            installer_type: None,
            installer_scope: None,
        }
    }

    /// True when the upstream registry supplied a strong digest for the artifact.
    pub fn has_upstream_digest(&self) -> bool {
        self.expected_sha256.is_some() || self.expected_sha512.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FetchedArtifact {
    pub adapter: AdapterKind,
    pub package_id: String,
    pub version: String,
    pub local_path: PathBuf,
    /// SHA-256 of the bytes on disk; `None` in dry runs where nothing was downloaded.
    pub computed_sha256: Option<String>,
    pub size_bytes: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub dry_run: bool,
    pub working_dir: PathBuf,
    pub isolated_env: Option<PathBuf>,
    /// Cancels the in-flight OS command when the user hits Stop / Esc.
    #[serde(skip)]
    pub cancel: CancellationToken,
    /// Called when a long command is still running with no new output (R5 stall watch).
    #[serde(skip)]
    pub on_progress: Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>,
}

impl std::fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("dry_run", &self.dry_run)
            .field("working_dir", &self.working_dir)
            .field("isolated_env", &self.isolated_env)
            .field("on_progress", &self.on_progress.as_ref().map(|_| "Some(..)"))
            .finish_non_exhaustive()
    }
}

impl PartialEq for ExecutionContext {
    fn eq(&self, other: &Self) -> bool {
        self.dry_run == other.dry_run
            && self.working_dir == other.working_dir
            && self.isolated_env == other.isolated_env
    }
}

impl Eq for ExecutionContext {}

impl ExecutionContext {
    pub fn new(dry_run: bool, working_dir: PathBuf, isolated_env: Option<PathBuf>) -> Self {
        Self {
            dry_run,
            working_dir,
            isolated_env,
            cancel: CancellationToken::new(),
            on_progress: None,
        }
    }

    pub fn with_cancel(mut self, cancel: CancellationToken) -> Self {
        self.cancel = cancel;
        self
    }

    pub fn with_progress(
        mut self,
        on_progress: std::sync::Arc<dyn Fn(String) + Send + Sync>,
    ) -> Self {
        self.on_progress = Some(on_progress);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionReceipt {
    pub adapter: AdapterKind,
    pub package_id: String,
    pub version: String,
    pub dry_run: bool,
    pub changes_made: Vec<String>,
    pub rollback_hint: Option<String>,
    /// Exact argv (binary first) that reverses the install, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uninstall_argv: Option<Vec<String>>,
    /// Files or folders the install created that rollback should delete.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub created_paths: Vec<PathBuf>,
    /// Folders the executor must add to the User PATH after the install.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path_entries: Vec<PathBuf>,
    /// Raw user PATH before a winget install (R6 rewind).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_path_before: Option<String>,
    /// PATH entries the install added (diff of before/after).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path_entries_added: Vec<String>,
    /// Primary install folder when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_location: Option<PathBuf>,
}

impl ExecutionReceipt {
    pub fn new(
        adapter: AdapterKind,
        package_id: impl Into<String>,
        version: impl Into<String>,
        dry_run: bool,
        changes_made: Vec<String>,
        rollback_hint: Option<String>,
    ) -> Self {
        Self {
            adapter,
            package_id: package_id.into(),
            version: version.into(),
            dry_run,
            changes_made,
            rollback_hint,
            uninstall_argv: None,
            created_paths: Vec::new(),
            path_entries: Vec::new(),
            user_path_before: None,
            path_entries_added: Vec::new(),
            install_location: None,
        }
    }

    pub fn with_uninstall_argv(mut self, argv: Vec<String>) -> Self {
        self.uninstall_argv = Some(argv);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationResult {
    pub passed: bool,
    pub command_run: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl VerificationResult {
    pub fn failed(command_run: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            passed: false,
            command_run: command_run.into(),
            exit_code: None,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("package not found: {package_id} in {adapter:?}")]
    PackageNotFound {
        adapter: AdapterKind,
        package_id: String,
    },
    #[error("unapproved domain in URL `{url}`")]
    UnapprovedDomain { url: String },
    #[error("hash mismatch for {file}: expected {expected}, got {actual}")]
    HashMismatch {
        file: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("network/registry request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("execution failed (exit {exit_code:?}): {stderr}")]
    CommandFailed {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("adapter error: {0}")]
    Other(String),
}

/// Unified interface implemented by `Winget`, `PyPI`, `Npm`, `Modrinth`, `Cargo`,
/// `GitHubReleases`, `OfficialVendorUrl`.
#[async_trait]
pub trait SourceAdapter: Send + Sync {
    fn kind(&self) -> AdapterKind;

    async fn resolve_metadata(
        &self,
        package_id: &str,
        version: Option<&str>,
    ) -> Result<PackageMetadata, AdapterError>;

    async fn fetch_artifact(
        &self,
        metadata: &PackageMetadata,
        destination: &Path,
        ctx: &ExecutionContext,
    ) -> Result<FetchedArtifact, AdapterError>;

    async fn install(
        &self,
        metadata: &PackageMetadata,
        artifact: Option<&FetchedArtifact>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, AdapterError>;

    async fn verify(
        &self,
        metadata: &PackageMetadata,
        ctx: &ExecutionContext,
    ) -> Result<VerificationResult, AdapterError>;
}

/// Registry holding active `SourceAdapter` implementations keyed by `AdapterKind`.
#[derive(Clone, Default)]
pub struct AdapterRegistry {
    adapters: HashMap<AdapterKind, Arc<dyn SourceAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Creates a registry using real system adapters, backed by `OsProvider`.
    pub fn with_system_defaults(os_provider: Arc<dyn crate::os::OsProvider>) -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(winget::WingetAdapter::new(os_provider.clone())));
        reg.register(Arc::new(pypi::PyPIAdapter::new(os_provider.clone())));
        reg.register(Arc::new(npm::NpmAdapter::new(os_provider.clone())));
        reg.register(Arc::new(modrinth::ModrinthAdapter::new(os_provider.clone())));
        reg.register(Arc::new(cargo::CargoAdapter::new(os_provider.clone())));
        reg.register(Arc::new(vendor::VendorUrlAdapter::new(os_provider.clone())));
        reg.register(Arc::new(github::GitHubReleasesAdapter::new(os_provider)));
        reg
    }

    /// System adapters where GitHub API calls carry the user's token (read per request).
    pub fn with_github_token(
        os_provider: Arc<dyn crate::os::OsProvider>,
        github_token: crate::config::SecretProvider,
    ) -> Self {
        let mut reg = Self::with_system_defaults(os_provider.clone());
        reg.register(Arc::new(
            github::GitHubReleasesAdapter::new(os_provider).with_token_provider(github_token),
        ));
        reg
    }

    pub fn register(&mut self, adapter: Arc<dyn SourceAdapter>) {
        self.adapters.insert(adapter.kind(), adapter);
    }

    pub fn get(&self, kind: AdapterKind) -> Option<Arc<dyn SourceAdapter>> {
        self.adapters.get(&kind).cloned()
    }
}

/// Shared HTTP client used by the registry adapters.
pub(crate) fn registry_http_client(timeout_secs: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("SetupPal-Orchestrator/0.0.1")
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub(crate) fn age_days_since(iso: Option<&str>) -> Option<u64> {
    let parsed = DateTime::parse_from_rfc3339(iso?).ok()?;
    let days = Utc::now()
        .signed_duration_since(parsed.with_timezone(&Utc))
        .num_days();
    Some(days.max(0) as u64)
}

pub(crate) fn parse_rfc3339(iso: Option<&str>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(iso?)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Last path segment of a URL, percent-decoded, when it is a plain file name.
pub(crate) fn file_name_from_url(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw).ok()?;
    let last = parsed.path_segments()?.next_back()?;
    let bytes = last.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    let name = String::from_utf8(out).ok()?;
    let valid = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':'])
        && crate::safety::policy::contains_shell_metacharacters(&name).is_none();
    valid.then_some(name)
}

/// Downloads an allowlisted `https` URL to `destination`, streaming through a `.partial`
/// file and refusing bodies larger than `max_bytes`. Returns the number of bytes written.
pub(crate) async fn download_to_file(
    http: &reqwest::Client,
    url: &str,
    destination: &Path,
    max_bytes: u64,
) -> Result<u64, AdapterError> {
    if !is_url_domain_allowlisted(url).unwrap_or(false) {
        return Err(AdapterError::UnapprovedDomain {
            url: url.to_string(),
        });
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut resp = http.get(url).send().await?.error_for_status()?;
    if let Some(len) = resp.content_length() {
        if len > max_bytes {
            return Err(AdapterError::Other(format!(
                "download of {url} is {len} bytes, above the {max_bytes}-byte limit"
            )));
        }
    }
    let final_host_ok = is_url_domain_allowlisted(resp.url().as_str()).unwrap_or(false);
    if !final_host_ok {
        return Err(AdapterError::UnapprovedDomain {
            url: resp.url().to_string(),
        });
    }

    let mut partial = destination.as_os_str().to_owned();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    let mut file = std::fs::File::create(&partial)?;
    let mut written: u64 = 0;
    let result: Result<(), AdapterError> = async {
        while let Some(chunk) = resp.chunk().await? {
            written += chunk.len() as u64;
            if written > max_bytes {
                return Err(AdapterError::Other(format!(
                    "download of {url} exceeded the {max_bytes}-byte limit"
                )));
            }
            file.write_all(&chunk)?;
        }
        file.flush()?;
        Ok(())
    }
    .await;
    drop(file);

    if let Err(e) = result {
        let _ = std::fs::remove_file(&partial);
        return Err(e);
    }
    std::fs::rename(&partial, destination)?;
    Ok(written)
}

/// Checks a downloaded file against every digest that is known. Deletes the file and returns
/// `HashMismatch` on any mismatch. Returns the computed SHA-256 on success.
pub(crate) fn enforce_digests(
    path: &Path,
    expected_sha256: Option<&str>,
    expected_sha512: Option<&str>,
) -> Result<String, AdapterError> {
    let actual_256 = compute_file_sha256(path)?;
    if let Some(expected) = expected_sha256 {
        let expected = expected.trim().to_ascii_lowercase();
        if actual_256 != expected {
            let _ = std::fs::remove_file(path);
            return Err(AdapterError::HashMismatch {
                file: path.to_path_buf(),
                expected,
                actual: actual_256,
            });
        }
    }
    if let Some(expected) = expected_sha512 {
        let expected = expected.trim().to_ascii_lowercase();
        let actual_512 = compute_file_sha512(path)?;
        if actual_512 != expected {
            let _ = std::fs::remove_file(path);
            return Err(AdapterError::HashMismatch {
                file: path.to_path_buf(),
                expected,
                actual: actual_512,
            });
        }
    }
    Ok(actual_256)
}

/// Shared `fetch_artifact` body: dry runs report what would happen; live runs download
/// and enforce every known digest.
pub(crate) async fn fetch_with_digests(
    http: &reqwest::Client,
    metadata: &PackageMetadata,
    destination: &Path,
    ctx: &ExecutionContext,
) -> Result<FetchedArtifact, AdapterError> {
    let download_url = metadata.download_url.as_deref().ok_or_else(|| {
        AdapterError::Other(format!(
            "{:?} metadata for {} does not contain a download URL",
            metadata.adapter, metadata.package_id
        ))
    })?;
    if !is_url_domain_allowlisted(download_url).unwrap_or(false) {
        return Err(AdapterError::UnapprovedDomain {
            url: download_url.to_string(),
        });
    }

    if ctx.dry_run {
        return Ok(FetchedArtifact {
            adapter: metadata.adapter,
            package_id: metadata.package_id.clone(),
            version: metadata.resolved_version.clone(),
            local_path: destination.to_path_buf(),
            computed_sha256: None,
            size_bytes: 0,
        });
    }

    let size = download_to_file(http, download_url, destination, MAX_DOWNLOAD_BYTES).await?;
    let computed = enforce_digests(
        destination,
        metadata.expected_sha256.as_deref(),
        metadata.expected_sha512.as_deref(),
    )?;

    Ok(FetchedArtifact {
        adapter: metadata.adapter,
        package_id: metadata.package_id.clone(),
        version: metadata.resolved_version.clone(),
        local_path: destination.to_path_buf(),
        computed_sha256: Some(computed),
        size_bytes: size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn enforce_digests_accepts_match_and_deletes_on_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.bin");
        std::fs::write(&file, b"setuppal").unwrap();
        let good = hex::encode(Sha256::digest(b"setuppal"));

        assert_eq!(enforce_digests(&file, Some(&good), None).unwrap(), good);
        assert!(file.exists());

        let bad = "a".repeat(64);
        let err = enforce_digests(&file, Some(&bad), None).unwrap_err();
        assert!(matches!(err, AdapterError::HashMismatch { .. }));
        assert!(!file.exists());
    }

    #[test]
    fn file_name_from_url_decodes_and_rejects_traversal() {
        assert_eq!(
            file_name_from_url("https://cdn.modrinth.com/data/x/sodium-fabric-0.6.13%2Bmc1.21.1.jar").as_deref(),
            Some("sodium-fabric-0.6.13+mc1.21.1.jar")
        );
        assert_eq!(file_name_from_url("https://example.com/a/%2E%2E"), None);
        assert_eq!(file_name_from_url("https://example.com/a/x%5C..%5Cy"), None);
    }

    #[test]
    fn enforce_digests_checks_sha512_too() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("b.bin");
        std::fs::write(&file, b"mod").unwrap();
        let err = enforce_digests(&file, None, Some(&"0".repeat(128))).unwrap_err();
        assert!(matches!(err, AdapterError::HashMismatch { .. }));
        assert!(!file.exists());
    }
}
