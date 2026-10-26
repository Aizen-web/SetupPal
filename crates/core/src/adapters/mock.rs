use std::collections::HashMap;
use std::path::Path;
use async_trait::async_trait;
use chrono::Utc;

use crate::adapters::{
    AdapterError, ExecutionContext, ExecutionReceipt, FetchedArtifact, PackageMetadata,
    SourceAdapter, VerificationResult,
};
use crate::types::AdapterKind;

/// In-memory deterministic `SourceAdapter` used in unit tests and offline mode.
/// Never executes real system-wide installs.
#[derive(Debug, Clone)]
pub struct MockAdapter {
    kind: AdapterKind,
    packages: HashMap<String, PackageMetadata>,
}

impl MockAdapter {
    pub fn new(kind: AdapterKind) -> Self {
        Self {
            kind,
            packages: HashMap::new(),
        }
    }

    pub fn with_defaults_for(kind: AdapterKind) -> Self {
        let mut adapter = Self::new(kind);
        // Seed known well-established packages with high age, high download counts, and valid SHA-256 hashes
        let established = [
            ("LLVM.LLVM", "18.1.8", "https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.8/LLVM-18.1.8-win64.exe"),
            ("Kitware.CMake", "3.30.2", "https://github.com/Kitware/CMake/releases/download/v3.30.2/cmake-3.30.2-windows-x86_64.msi"),
            ("Git.Git", "2.46.0", "https://github.com/git-for-windows/git/releases/download/v2.46.0.windows.1/Git-2.46.0-64-bit.exe"),
            ("OpenJS.NodeJS.LTS", "20.17.0", "https://nodejs.org/dist/v20.17.0/node-v20.17.0-x64.msi"),
            ("EclipseAdoptium.Temurin.21.JDK", "21.0.4+7", "https://api.adoptium.net/v3/installer/latest/21/ga/windows/x64/jdk/hotspot/normal/eclipse"),
            ("requests", "2.32.3", "https://files.pythonhosted.org/packages/requests-2.32.3-py3-none-any.whl"),
            ("numpy", "2.1.1", "https://files.pythonhosted.org/packages/numpy-2.1.1-cp312-win_amd64.whl"),
            ("pandas", "2.2.3", "https://files.pythonhosted.org/packages/pandas-2.2.3-cp312-win_amd64.whl"),
            ("colorama", "0.4.6", "https://files.pythonhosted.org/packages/colorama-0.4.6-py2.py3-none-any.whl"),
            ("typescript", "5.6.2", "https://registry.npmjs.org/typescript/-/typescript-5.6.2.tgz"),
            ("sodium", "mc1.21-0.6.0", "https://cdn.modrinth.com/data/AANobbMI/versions/mc1.21-0.6.0/sodium-fabric-0.6.0%2Bmc1.21.jar"),
            ("lithium", "mc1.21-0.13.0", "https://cdn.modrinth.com/data/gvQqBUqZ/versions/mc1.21-0.13.0/lithium-fabric-0.13.0%2Bmc1.21.jar"),
            ("ferrite-core", "7.0.0-fabric", "https://cdn.modrinth.com/data/uXXizFIs/versions/7.0.0-fabric/ferritecore-7.0.0-fabric.jar"),
        ];

        for (pkg, ver, url) in established {
            adapter.insert_package(PackageMetadata {
                adapter: kind,
                package_id: pkg.to_string(),
                resolved_version: ver.to_string(),
                published_at: Some(Utc::now()),
                age_days: Some(420),
                download_count: Some(2_500_000),
                download_url: Some(url.to_string()),
                expected_sha256: Some(
                    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                        .to_string(),
                ),
                expected_sha512: None,
                install_script_snippet: None,
            });
        }
        adapter
    }

    pub fn insert_package(&mut self, metadata: PackageMetadata) {
        self.packages
            .insert(metadata.package_id.to_ascii_lowercase(), metadata);
    }
}

#[async_trait]
impl SourceAdapter for MockAdapter {
    fn kind(&self) -> AdapterKind {
        self.kind
    }

    async fn resolve_metadata(
        &self,
        package_id: &str,
        version: Option<&str>,
    ) -> Result<PackageMetadata, AdapterError> {
        if let Some(meta) = self.packages.get(&package_id.to_ascii_lowercase()) {
            let mut cloned = meta.clone();
            if let Some(v) = version {
                cloned.resolved_version = v.to_string();
            }
            return Ok(cloned);
        }

        // Default synthetic metadata for unknown packages (young/low-download so SafetyEngine flags Yellow)
        Ok(PackageMetadata {
            adapter: self.kind,
            package_id: package_id.to_string(),
            resolved_version: version.unwrap_or("0.1.0").to_string(),
            published_at: Some(Utc::now()),
            age_days: Some(5),
            download_count: Some(42),
            download_url: None,
            expected_sha256: None,
            expected_sha512: None,
            install_script_snippet: None,
        })
    }

    async fn fetch_artifact(
        &self,
        metadata: &PackageMetadata,
        destination: &Path,
        _ctx: &ExecutionContext,
    ) -> Result<FetchedArtifact, AdapterError> {
        Ok(FetchedArtifact {
            adapter: self.kind,
            package_id: metadata.package_id.clone(),
            version: metadata.resolved_version.clone(),
            local_path: destination.to_path_buf(),
            computed_sha256: metadata
                .expected_sha256
                .clone()
                .unwrap_or_else(|| "0".repeat(64)),
            size_bytes: 1024,
        })
    }

    async fn install(
        &self,
        metadata: &PackageMetadata,
        _artifact: Option<&FetchedArtifact>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, AdapterError> {
        Ok(ExecutionReceipt {
            adapter: self.kind,
            package_id: metadata.package_id.clone(),
            version: metadata.resolved_version.clone(),
            dry_run: ctx.dry_run,
            changes_made: vec![format!(
                "Mock installed {}@{}",
                metadata.package_id, metadata.resolved_version
            )],
            rollback_hint: Some(format!("uninstall {}", metadata.package_id)),
        })
    }

    async fn verify(
        &self,
        metadata: &PackageMetadata,
        _ctx: &ExecutionContext,
    ) -> Result<VerificationResult, AdapterError> {
        Ok(VerificationResult {
            passed: true,
            command_run: format!("mock-verify {}", metadata.package_id),
            exit_code: Some(0),
            stdout: format!("{} {} verified OK", metadata.package_id, metadata.resolved_version),
            stderr: String::new(),
        })
    }
}
