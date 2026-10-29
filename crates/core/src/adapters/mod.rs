pub mod github;
pub mod modrinth;
pub mod npm;
pub mod pypi;
pub mod winget;

pub use github::GitHubReleasesAdapter;
pub use modrinth::ModrinthAdapter;
pub use npm::NpmAdapter;
pub use pypi::PyPIAdapter;
pub use winget::WingetAdapter;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::AdapterKind;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageMetadata {
    pub adapter: AdapterKind,
    pub package_id: String,
    pub resolved_version: String,
    pub published_at: Option<DateTime<Utc>>,
    pub age_days: Option<u64>,
    pub download_count: Option<u64>,
    pub download_url: Option<String>,
    pub expected_sha256: Option<String>,
    pub expected_sha512: Option<String>,
    pub install_script_snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FetchedArtifact {
    pub adapter: AdapterKind,
    pub package_id: String,
    pub version: String,
    pub local_path: PathBuf,
    pub computed_sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionContext {
    pub dry_run: bool,
    pub working_dir: PathBuf,
    pub isolated_env: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionReceipt {
    pub adapter: AdapterKind,
    pub package_id: String,
    pub version: String,
    pub dry_run: bool,
    pub changes_made: Vec<String>,
    pub rollback_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationResult {
    pub passed: bool,
    pub command_run: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
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

/// Unified interface implemented by `Winget`, `PyPI`, `Npm`, `Modrinth`,
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

    /// Creates a registry using real system adapters (e.g. `WingetAdapter`, `PyPIAdapter`,
    /// `NpmAdapter`, `ModrinthAdapter`, `GitHubReleasesAdapter`), backed by `OsProvider`.
    pub fn with_system_defaults(os_provider: Arc<dyn crate::os::OsProvider>) -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(winget::WingetAdapter::new(os_provider.clone())));
        reg.register(Arc::new(pypi::PyPIAdapter::new(os_provider.clone())));
        reg.register(Arc::new(npm::NpmAdapter::new(os_provider.clone())));
        reg.register(Arc::new(modrinth::ModrinthAdapter::new(os_provider.clone())));
        reg.register(Arc::new(github::GitHubReleasesAdapter::new(os_provider)));
        reg
    }

    pub fn register(&mut self, adapter: Arc<dyn SourceAdapter>) {
        self.adapters.insert(adapter.kind(), adapter);
    }

    pub fn get(&self, kind: AdapterKind) -> Option<Arc<dyn SourceAdapter>> {
        self.adapters.get(&kind).cloned()
    }
}
