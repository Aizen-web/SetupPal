use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use serde::Deserialize;

use crate::adapters::{
    age_days_since, fetch_with_digests, file_name_from_url, parse_rfc3339, registry_http_client,
    AdapterError, ExecutionContext, ExecutionReceipt, FetchedArtifact, PackageMetadata,
    SourceAdapter, VerificationResult,
};
use crate::os::OsProvider;
use crate::types::AdapterKind;

pub const DEFAULT_MODRINTH_BASE_URL: &str = "https://api.modrinth.com/v2";

/// Native adapter for the Modrinth Minecraft mod registry.
/// Modrinth publishes SHA-512 (and SHA-1) digests per file; SHA-512 is enforced on download.
pub struct ModrinthAdapter {
    http: reqwest::Client,
    base_url: String,
    mods_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Clone)]
struct ModrinthVersion {
    version_number: String,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    date_published: Option<String>,
    #[serde(default)]
    downloads: Option<u64>,
    #[serde(default)]
    files: Vec<ModrinthFile>,
}

#[derive(Debug, Deserialize, Clone)]
struct ModrinthFile {
    url: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: ModrinthHashes,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct ModrinthHashes {
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    sha512: Option<String>,
}

impl ModrinthAdapter {
    pub fn new(_os_provider: Arc<dyn OsProvider>) -> Self {
        Self {
            http: registry_http_client(30),
            base_url: DEFAULT_MODRINTH_BASE_URL.to_string(),
            mods_dir: crate::paths::minecraft_dir().map(|d| d.join("mods")),
        }
    }

    pub fn with_base_url(
        http: reqwest::Client,
        _os_provider: Arc<dyn OsProvider>,
        base_url: String,
    ) -> Self {
        Self {
            http,
            base_url,
            mods_dir: crate::paths::minecraft_dir().map(|d| d.join("mods")),
        }
    }

    /// Overrides the Minecraft `mods` folder used by `install` and `verify`.
    pub fn with_mods_dir(mut self, mods_dir: PathBuf) -> Self {
        self.mods_dir = Some(mods_dir);
        self
    }

    fn select_version<'a>(versions: &'a [ModrinthVersion], requested: Option<&str>) -> Option<&'a ModrinthVersion> {
        match requested {
            Some(v) if !v.is_empty() && v != "latest" => versions.iter().find(|x| x.version_number == v),
            _ => versions
                .iter()
                .find(|x| x.loaders.iter().any(|l| l.eq_ignore_ascii_case("fabric")))
                .or_else(|| versions.first()),
        }
    }

    fn installed_jar_path(&self, metadata: &PackageMetadata) -> Result<PathBuf, AdapterError> {
        let mods = self.mods_dir.clone().ok_or_else(|| {
            AdapterError::Other("Minecraft Java Edition data folder could not be located".to_string())
        })?;
        let name = metadata
            .download_url
            .as_deref()
            .and_then(file_name_from_url)
            .ok_or_else(|| AdapterError::Other(format!("no file name for mod {}", metadata.package_id)))?;
        Ok(mods.join(name))
    }
}

#[async_trait]
impl SourceAdapter for ModrinthAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::Modrinth
    }

    async fn resolve_metadata(
        &self,
        package_id: &str,
        version: Option<&str>,
    ) -> Result<PackageMetadata, AdapterError> {
        let url = format!("{}/project/{}/version", self.base_url, package_id);
        let response = self.http.get(&url).send().await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AdapterError::PackageNotFound {
                adapter: AdapterKind::Modrinth,
                package_id: package_id.to_string(),
            });
        }
        let versions: Vec<ModrinthVersion> = response.error_for_status()?.json().await?;

        let matched = Self::select_version(&versions, version).ok_or_else(|| AdapterError::PackageNotFound {
            adapter: AdapterKind::Modrinth,
            package_id: match version {
                Some(v) => format!("{package_id}@{v}"),
                None => package_id.to_string(),
            },
        })?;

        let file = matched
            .files
            .iter()
            .find(|f| f.primary)
            .or_else(|| matched.files.first());

        let mut meta = PackageMetadata::bare(AdapterKind::Modrinth, package_id, matched.version_number.clone());
        meta.published_at = parse_rfc3339(matched.date_published.as_deref());
        meta.age_days = age_days_since(matched.date_published.as_deref());
        meta.download_count = matched.downloads;
        meta.download_url = file.map(|f| f.url.clone());
        meta.expected_sha256 = file.and_then(|f| f.hashes.sha256.clone());
        meta.expected_sha512 = file.and_then(|f| f.hashes.sha512.clone());
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

    /// Places the mod jar into the Minecraft `mods` folder (downloading it when no
    /// artifact was fetched by an earlier step).
    async fn install(
        &self,
        metadata: &PackageMetadata,
        artifact: Option<&FetchedArtifact>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, AdapterError> {
        if let Some(a) = artifact {
            return Ok(ExecutionReceipt::new(
                AdapterKind::Modrinth,
                metadata.package_id.clone(),
                metadata.resolved_version.clone(),
                ctx.dry_run,
                vec![format!("Mod {} is at {}", metadata.package_id, a.local_path.display())],
                Some(format!("delete {}", a.local_path.display())),
            ));
        }

        let dest = self.installed_jar_path(metadata)?;
        if ctx.dry_run {
            return Ok(ExecutionReceipt::new(
                AdapterKind::Modrinth,
                metadata.package_id.clone(),
                metadata.resolved_version.clone(),
                true,
                vec![format!("[Dry Run] Would download {} into {}", metadata.package_id, dest.display())],
                Some(format!("delete {}", dest.display())),
            ));
        }
        if !metadata.has_upstream_digest() {
            return Err(AdapterError::Other(format!(
                "Modrinth published no checksum for {}; refusing an unverified download",
                metadata.package_id
            )));
        }

        let fetched = fetch_with_digests(&self.http, metadata, &dest, ctx).await?;
        let mut receipt = ExecutionReceipt::new(
            AdapterKind::Modrinth,
            metadata.package_id.clone(),
            metadata.resolved_version.clone(),
            false,
            vec![format!(
                "Downloaded {} {} to {} (checksum verified)",
                metadata.package_id,
                metadata.resolved_version,
                fetched.local_path.display()
            )],
            Some(format!("delete {}", fetched.local_path.display())),
        );
        receipt.created_paths.push(fetched.local_path);
        Ok(receipt)
    }

    async fn verify(
        &self,
        metadata: &PackageMetadata,
        _ctx: &ExecutionContext,
    ) -> Result<VerificationResult, AdapterError> {
        let dest = match self.installed_jar_path(metadata) {
            Ok(d) => d,
            Err(e) => return Ok(VerificationResult::failed("verify_mod_file", e.to_string())),
        };
        let exists = dest.is_file();
        Ok(VerificationResult {
            passed: exists,
            command_run: format!("verify_mod_file {}", dest.display()),
            exit_code: Some(if exists { 0 } else { 1 }),
            stdout: if exists {
                format!("Mod file present: {}", dest.display())
            } else {
                String::new()
            },
            stderr: if exists {
                String::new()
            } else {
                format!("Mod file not found: {}", dest.display())
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(number: &str, loader: &str) -> ModrinthVersion {
        ModrinthVersion {
            version_number: number.to_string(),
            loaders: vec![loader.to_string()],
            date_published: None,
            downloads: None,
            files: Vec::new(),
        }
    }

    #[test]
    fn pinned_version_must_match_exactly() {
        let versions = vec![version("2.0-neoforge", "neoforge"), version("1.0-fabric", "fabric")];
        assert!(ModrinthAdapter::select_version(&versions, Some("9.9")).is_none());
        assert_eq!(
            ModrinthAdapter::select_version(&versions, Some("2.0-neoforge")).unwrap().version_number,
            "2.0-neoforge"
        );
        assert_eq!(
            ModrinthAdapter::select_version(&versions, None).unwrap().version_number,
            "1.0-fabric"
        );
    }
}
