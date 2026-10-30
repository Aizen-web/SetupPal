use std::path::Path;
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

pub const DEFAULT_NPM_BASE_URL: &str = "https://registry.npmjs.org";

/// Native adapter for the Node Package Manager (npm) registry.
/// Installs run in the execution working directory (the user's project folder).
pub struct NpmAdapter {
    http: reqwest::Client,
    os_provider: Arc<dyn OsProvider>,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct NpmResponse {
    name: String,
    #[serde(rename = "dist-tags", default)]
    dist_tags: std::collections::HashMap<String, String>,
    #[serde(default)]
    time: std::collections::HashMap<String, String>,
    #[serde(default)]
    versions: std::collections::HashMap<String, NpmVersionInfo>,
}

#[derive(Debug, Deserialize, Clone)]
struct NpmVersionInfo {
    dist: NpmDist,
    #[serde(default)]
    scripts: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
struct NpmDist {
    tarball: String,
    #[serde(default)]
    integrity: Option<String>,
}

/// Converts an npm `sha512-<base64>` integrity string into lowercase hex.
fn integrity_to_sha512_hex(integrity: &str) -> Option<String> {
    let b64 = integrity.strip_prefix("sha512-")?;
    let bytes = decode_base64(b64)?;
    (bytes.len() == 64).then(|| hex::encode(bytes))
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits = 0;
    for c in input.bytes() {
        let val = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

impl NpmAdapter {
    pub fn new(os_provider: Arc<dyn OsProvider>) -> Self {
        Self {
            http: registry_http_client(30),
            os_provider,
            base_url: DEFAULT_NPM_BASE_URL.to_string(),
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

    fn package_spec(metadata: &PackageMetadata) -> String {
        if metadata.resolved_version.is_empty() || metadata.resolved_version == "latest" {
            metadata.package_id.clone()
        } else {
            format!("{}@{}", metadata.package_id, metadata.resolved_version)
        }
    }

    fn install_script_snippet(scripts: &std::collections::HashMap<String, String>) -> Option<String> {
        let lifecycle: Vec<String> = ["preinstall", "install", "postinstall"]
            .iter()
            .filter_map(|k| scripts.get(*k).map(|v| format!("{k}: {v}")))
            .collect();
        (!lifecycle.is_empty()).then(|| lifecycle.join("\n"))
    }
}

#[async_trait]
impl SourceAdapter for NpmAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::Npm
    }

    async fn resolve_metadata(
        &self,
        package_id: &str,
        version: Option<&str>,
    ) -> Result<PackageMetadata, AdapterError> {
        let encoded = package_id.replace('/', "%2F");
        let url = format!("{}/{}", self.base_url, encoded);
        let response = self.http.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AdapterError::PackageNotFound {
                adapter: AdapterKind::Npm,
                package_id: package_id.to_string(),
            });
        }
        let body: NpmResponse = response.error_for_status()?.json().await?;

        let resolved_version = match version {
            Some(v) if !v.is_empty() && v != "latest" => v.to_string(),
            _ => body.dist_tags.get("latest").cloned().ok_or_else(|| {
                AdapterError::Other(format!("npm package {package_id} has no `latest` dist-tag"))
            })?,
        };

        let ver_info = body.versions.get(&resolved_version).ok_or_else(|| AdapterError::PackageNotFound {
            adapter: AdapterKind::Npm,
            package_id: format!("{package_id}@{resolved_version}"),
        })?;

        let published = body.time.get(&resolved_version).map(String::as_str);
        let mut meta = PackageMetadata::bare(AdapterKind::Npm, body.name.clone(), resolved_version.clone());
        meta.published_at = parse_rfc3339(published);
        meta.age_days = age_days_since(published);
        meta.download_url = Some(ver_info.dist.tarball.clone());
        meta.expected_sha512 = ver_info
            .dist
            .integrity
            .as_deref()
            .and_then(integrity_to_sha512_hex);
        meta.install_script_snippet = Self::install_script_snippet(&ver_info.scripts);
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
        let pkg_spec = Self::package_spec(metadata);
        let project = ctx.working_dir.display().to_string();
        let rollback_hint = Some(format!("npm uninstall {} --prefix {}", metadata.package_id, project));

        if ctx.dry_run {
            return Ok(ExecutionReceipt::new(
                AdapterKind::Npm,
                metadata.package_id.clone(),
                metadata.resolved_version.clone(),
                true,
                vec![format!("[Dry Run] Would execute in {project}: npm install {pkg_spec}")],
                rollback_hint,
            ));
        }

        if !ctx.working_dir.is_dir() {
            return Err(AdapterError::Other(format!(
                "project folder {project} does not exist"
            )));
        }

        let paths = self
            .os_provider
            .refresh_and_get_path()
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;
        let npm_bin = self
            .os_provider
            .which_in_paths("npm", &paths)
            .ok_or_else(|| AdapterError::Other("npm executable not found on host".to_string()))?;

        let args = vec![
            "install".to_string(),
            pkg_spec,
            "--no-audit".to_string(),
            "--no-fund".to_string(),
        ];
        let out = self
            .os_provider
            .run_command_cancellable(
                &npm_bin,
                &args,
                &paths,
                Some(&ctx.working_dir),
                Some(&ctx.cancel),
            )
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
            AdapterKind::Npm,
            metadata.package_id.clone(),
            metadata.resolved_version.clone(),
            false,
            vec![format!(
                "Installed {} ({}) into {}\\node_modules via npm",
                metadata.package_id, metadata.resolved_version, project
            )],
            rollback_hint,
        )
        .with_uninstall_argv(vec![
            "npm".to_string(),
            "uninstall".to_string(),
            metadata.package_id.clone(),
            "--prefix".to_string(),
            project,
        ]))
    }

    async fn verify(
        &self,
        metadata: &PackageMetadata,
        ctx: &ExecutionContext,
    ) -> Result<VerificationResult, AdapterError> {
        let command_run = format!("npm ls {} --depth=0", metadata.package_id);
        let paths = self
            .os_provider
            .refresh_and_get_path()
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;
        let Some(npm_bin) = self.os_provider.which_in_paths("npm", &paths) else {
            return Ok(VerificationResult::failed(command_run, "npm executable not found"));
        };

        let args = vec![
            "ls".to_string(),
            metadata.package_id.clone(),
            "--depth=0".to_string(),
        ];
        let out = self
            .os_provider
            .run_command_in_dir(&npm_bin, &args, &paths, Some(&ctx.working_dir))
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        let passed = out.exit_code == Some(0) && out.stdout.contains(&format!("{}@", metadata.package_id));
        Ok(VerificationResult {
            passed,
            command_run: format!("{command_run} (in {})", ctx.working_dir.display()),
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
    use std::path::PathBuf;

    #[test]
    fn integrity_decodes_to_sha512_hex() {
        // sha512 of the empty string, base64-encoded as npm would publish it.
        let integrity = "sha512-z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg/SpIdNs6c5H0NE8XYXysP+DGNKHfuwvY7kxvUdBeoGlODJ6+SfaPg==";
        let hexed = integrity_to_sha512_hex(integrity).unwrap();
        assert!(hexed.starts_with("cf83e1357eefb8bd"));
        assert_eq!(hexed.len(), 128);
        assert!(integrity_to_sha512_hex("sha1-abc").is_none());
    }

    #[tokio::test]
    async fn install_runs_in_project_dir() {
        let project = tempfile::tempdir().unwrap();
        let os = Arc::new(
            MockOsProvider::new()
                .with_binary("npm", "C:/mock/npm.cmd")
                .on("npm", &["install"], ok("added 1 package")),
        );
        let adapter = NpmAdapter::new(os.clone());
        let meta = PackageMetadata::bare(AdapterKind::Npm, "typescript", "5.6.2");
        let ctx = ExecutionContext::new(false, project.path().to_path_buf(), None,
        );
        let receipt = adapter.install(&meta, None, &ctx).await.unwrap();
        let calls = os.calls();
        assert_eq!(calls[0].cwd.as_deref(), Some(project.path()));
        assert_eq!(calls[0].args[1], "typescript@5.6.2");
        let argv = receipt.uninstall_argv.unwrap();
        assert!(argv.contains(&"--prefix".to_string()));
    }

    #[tokio::test]
    async fn verify_requires_package_in_ls_output() {
        let os = MockOsProvider::new()
            .with_binary("npm", "C:/mock/npm.cmd")
            .on("npm", &["ls"], ok("my-app@1.0.0 C:\\proj\n`-- (empty)"));
        let adapter = NpmAdapter::new(Arc::new(os));
        let meta = PackageMetadata::bare(AdapterKind::Npm, "typescript", "5.6.2");
        let ctx = ExecutionContext::new(false, PathBuf::from("."), None,
        );
        assert!(!adapter.verify(&meta, &ctx).await.unwrap().passed);
    }
}
