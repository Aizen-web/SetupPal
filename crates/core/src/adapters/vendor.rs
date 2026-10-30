use std::path::Path;
use std::sync::Arc;
use async_trait::async_trait;
use serde::Deserialize;

use crate::adapters::{
    fetch_with_digests, registry_http_client, AdapterError, ExecutionContext, ExecutionReceipt,
    FetchedArtifact, PackageMetadata, SourceAdapter, VerificationResult,
};
use crate::os::OsProvider;
use crate::safety::policy::{is_url_domain_allowlisted, is_valid_sha256_hex};
use crate::types::AdapterKind;

pub const FABRIC_INSTALLER_ID: &str = "fabric-installer";
pub const FABRIC_META_INSTALLER_URL: &str = "https://meta.fabricmc.net/v2/versions/installer";

/// Downloads vendor artifacts from allowlisted domains only.
///
/// `id` is either a known vendor artifact key (currently `fabric-installer`, resolved through
/// Fabric's meta API and its maven `.sha256` / `.sha512` sidecars) or a literal `https` URL on
/// an allowlisted host. Downloads are size-capped and every known digest is enforced.
pub struct VendorUrlAdapter {
    http: reqwest::Client,
    fabric_meta_url: String,
}

#[derive(Debug, Deserialize)]
struct FabricInstallerVersion {
    url: String,
    version: String,
    #[serde(default)]
    stable: bool,
}

impl VendorUrlAdapter {
    pub fn new(_os_provider: Arc<dyn OsProvider>) -> Self {
        Self {
            http: registry_http_client(60),
            fabric_meta_url: FABRIC_META_INSTALLER_URL.to_string(),
        }
    }

    async fn fetch_sidecar_digest(&self, url: &str, ext: &str, hex_len: usize) -> Option<String> {
        let resp = self.http.get(format!("{url}.{ext}")).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let text = resp.text().await.ok()?;
        let digest = text.split_whitespace().next()?.to_ascii_lowercase();
        (digest.len() == hex_len && digest.chars().all(|c| c.is_ascii_hexdigit())).then_some(digest)
    }

    async fn resolve_fabric_installer(&self, version: Option<&str>) -> Result<PackageMetadata, AdapterError> {
        let versions: Vec<FabricInstallerVersion> = self
            .http
            .get(&self.fabric_meta_url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let chosen = match version {
            Some(v) if !v.is_empty() && v != "latest" => versions.iter().find(|x| x.version == v),
            _ => versions.iter().find(|x| x.stable).or_else(|| versions.first()),
        }
        .ok_or_else(|| AdapterError::PackageNotFound {
            adapter: AdapterKind::OfficialVendorUrl,
            package_id: format!("{FABRIC_INSTALLER_ID}@{}", version.unwrap_or("latest")),
        })?;

        if !is_url_domain_allowlisted(&chosen.url).unwrap_or(false) {
            return Err(AdapterError::UnapprovedDomain {
                url: chosen.url.clone(),
            });
        }

        let mut meta = PackageMetadata::bare(AdapterKind::OfficialVendorUrl, FABRIC_INSTALLER_ID, chosen.version.clone());
        meta.download_url = Some(chosen.url.clone());
        meta.expected_sha256 = self
            .fetch_sidecar_digest(&chosen.url, "sha256", 64)
            .await
            .filter(|d| is_valid_sha256_hex(d));
        meta.expected_sha512 = self.fetch_sidecar_digest(&chosen.url, "sha512", 128).await;
        meta.installer_type = Some("jar".to_string());
        Ok(meta)
    }
}

#[async_trait]
impl SourceAdapter for VendorUrlAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::OfficialVendorUrl
    }

    async fn resolve_metadata(
        &self,
        package_id: &str,
        version: Option<&str>,
    ) -> Result<PackageMetadata, AdapterError> {
        if package_id.eq_ignore_ascii_case(FABRIC_INSTALLER_ID) {
            return self.resolve_fabric_installer(version).await;
        }
        if package_id.starts_with("https://") || package_id.starts_with("http://") {
            match is_url_domain_allowlisted(package_id) {
                Ok(true) => {}
                Ok(false) => {
                    return Err(AdapterError::UnapprovedDomain {
                        url: package_id.to_string(),
                    })
                }
                Err(e) => return Err(AdapterError::Other(e)),
            }
            let mut meta = PackageMetadata::bare(
                AdapterKind::OfficialVendorUrl,
                package_id,
                version.unwrap_or("unversioned"),
            );
            meta.download_url = Some(package_id.to_string());
            return Ok(meta);
        }
        Err(AdapterError::PackageNotFound {
            adapter: AdapterKind::OfficialVendorUrl,
            package_id: package_id.to_string(),
        })
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
        _ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, AdapterError> {
        Err(AdapterError::Other(format!(
            "no unattended install path for vendor artifact {}; download it with download_artifact and run it with a typed vendor installer step",
            metadata.package_id
        )))
    }

    async fn verify(
        &self,
        metadata: &PackageMetadata,
        _ctx: &ExecutionContext,
    ) -> Result<VerificationResult, AdapterError> {
        Ok(VerificationResult::failed(
            format!("verify_vendor {}", metadata.package_id),
            "vendor artifacts are verified by their download hash or installer step",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::mock::MockOsProvider;

    fn adapter() -> VendorUrlAdapter {
        VendorUrlAdapter::new(Arc::new(MockOsProvider::new()))
    }

    #[tokio::test]
    async fn rejects_unapproved_hosts() {
        let err = adapter()
            .resolve_metadata("https://evil.example.com/tool.exe", None)
            .await
            .unwrap_err();
        assert!(matches!(err, AdapterError::UnapprovedDomain { .. }));
        assert!(adapter().resolve_metadata("http://maven.fabricmc.net/x.jar", None).await.is_err());
    }

    #[tokio::test]
    async fn allowlisted_url_resolves_without_inventing_a_digest() {
        let meta = adapter()
            .resolve_metadata("https://maven.fabricmc.net/net/fabricmc/x/1/x-1.jar", Some("1"))
            .await
            .unwrap();
        assert_eq!(meta.expected_sha256, None);
        assert!(!meta.has_upstream_digest());
    }

    #[tokio::test]
    async fn unknown_vendor_ids_are_not_found() {
        assert!(matches!(
            adapter().resolve_metadata("mystery-tool", None).await,
            Err(AdapterError::PackageNotFound { .. })
        ));
    }

    #[tokio::test]
    async fn install_never_claims_success() {
        let meta = PackageMetadata::bare(AdapterKind::OfficialVendorUrl, FABRIC_INSTALLER_ID, "1.1.2");
        let ctx = ExecutionContext {
            dry_run: true,
            working_dir: ".".into(),
            isolated_env: None,
        };
        assert!(adapter().install(&meta, None, &ctx).await.is_err());
    }
}
