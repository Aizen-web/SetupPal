use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use serde::Deserialize;

use crate::adapters::winget::WingetAdapter;
use crate::adapters::{
    age_days_since, fetch_with_digests, file_name_from_url, parse_rfc3339, registry_http_client,
    AdapterError, ExecutionContext, ExecutionReceipt, FetchedArtifact, PackageMetadata,
    SourceAdapter, VerificationResult,
};
use crate::os::OsProvider;
use crate::types::AdapterKind;

pub const DEFAULT_GITHUB_API_BASE_URL: &str = "https://api.github.com";

/// GitHub projects that publish an equivalent winget package. winget is preferred because
/// it carries reviewed manifests, silent switches, and clean uninstall.
pub const WINGET_EQUIVALENTS: &[(&str, &str)] = &[
    ("git-for-windows/git", "Git.Git"),
    ("llvm/llvm-project", "LLVM.LLVM"),
    ("kitware/cmake", "Kitware.CMake"),
    ("ninja-build/ninja", "Ninja-build.Ninja"),
    ("burntsushi/ripgrep", "BurntSushi.ripgrep.MSVC"),
    ("jqlang/jq", "jqlang.jq"),
    ("cli/cli", "GitHub.cli"),
    ("sharkdp/bat", "sharkdp.bat"),
    ("sharkdp/fd", "sharkdp.fd"),
    ("junegunn/fzf", "junegunn.fzf"),
    ("powershell/powershell", "Microsoft.PowerShell"),
    ("neovim/neovim", "Neovim.Neovim"),
    ("obsproject/obs-studio", "OBSProject.OBSStudio"),
    ("ip7z/7zip", "7zip.7zip"),
];

/// Returns the winget ID for a GitHub `owner/repo`, if one is known.
pub fn winget_id_for_repo(repo: &str) -> Option<&'static str> {
    let key = repo.trim().trim_end_matches('/').to_ascii_lowercase();
    WINGET_EQUIVALENTS
        .iter()
        .find(|(r, _)| *r == key)
        .map(|(_, id)| *id)
}

/// How a GitHub release asset can be installed without user interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubInstallMethod {
    Winget(&'static str),
    /// `msiexec /i <file> /qn /norestart` with per-user properties.
    Msi,
    /// Extract into `%LOCALAPPDATA%\SetupPal\tools\<repo>` and add it to the User PATH.
    Zip,
    /// Copy a standalone binary into the tools folder and add it to the User PATH.
    PortableExe,
    NoUnattendedPath(String),
}

/// Picks the install method for `repo` (`owner/name`) given the selected asset file name.
pub fn choose_install_method(repo: &str, asset_name: Option<&str>) -> GitHubInstallMethod {
    if let Some(id) = winget_id_for_repo(repo) {
        return GitHubInstallMethod::Winget(id);
    }
    let Some(name) = asset_name.map(str::to_ascii_lowercase) else {
        return GitHubInstallMethod::NoUnattendedPath(format!("release of {repo} has no Windows asset"));
    };
    if name.ends_with(".msi") {
        GitHubInstallMethod::Msi
    } else if name.ends_with(".zip") {
        GitHubInstallMethod::Zip
    } else if name.ends_with(".exe") {
        let installer_like = ["setup", "installer", "install"].iter().any(|k| name.contains(k));
        if installer_like {
            GitHubInstallMethod::NoUnattendedPath(format!(
                "{name} is an interactive installer with no known silent switches"
            ))
        } else {
            GitHubInstallMethod::PortableExe
        }
    } else {
        GitHubInstallMethod::NoUnattendedPath(format!("{name} is not an installable Windows asset"))
    }
}

/// Native adapter for GitHub Releases.
pub struct GitHubReleasesAdapter {
    http: reqwest::Client,
    os_provider: Arc<dyn OsProvider>,
    base_url: String,
}

#[derive(Debug, Deserialize, Clone)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize, Clone)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    download_count: u64,
    /// `sha256:<hex>` digest GitHub computes for uploaded assets.
    #[serde(default)]
    digest: Option<String>,
}

impl GitHubReleasesAdapter {
    pub fn new(os_provider: Arc<dyn OsProvider>) -> Self {
        Self {
            http: registry_http_client(60),
            os_provider,
            base_url: DEFAULT_GITHUB_API_BASE_URL.to_string(),
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

    fn normalize_repo_id(input: &str) -> Option<(String, String)> {
        let trimmed = input
            .trim()
            .trim_start_matches("https://github.com/")
            .trim_start_matches("http://github.com/");
        let mut parts = trimmed.split('/');
        let owner = parts.next().filter(|s| !s.is_empty())?;
        let repo = parts.next().filter(|s| !s.is_empty())?;
        Some((owner.to_string(), repo.to_string()))
    }

    fn select_best_windows_asset(assets: &[GitHubAsset]) -> Option<GitHubAsset> {
        let usable = |a: &&GitHubAsset| {
            let n = a.name.to_ascii_lowercase();
            n.ends_with(".exe") || n.ends_with(".msi") || n.ends_with(".zip")
        };
        let win_keywords = [
            "x86_64-pc-windows-msvc",
            "windows-x86_64",
            "windows-amd64",
            "win-x64",
            "win64",
            "x64.msi",
            "64-bit",
            "windows",
            "win.zip",
        ];
        for kw in win_keywords {
            if let Some(a) = assets
                .iter()
                .filter(usable)
                .find(|a| a.name.to_ascii_lowercase().contains(kw))
            {
                return Some(a.clone());
            }
        }
        None
    }

    fn repo_slug(metadata: &PackageMetadata) -> String {
        metadata
            .package_id
            .rsplit('/')
            .next()
            .unwrap_or(&metadata.package_id)
            .to_ascii_lowercase()
    }

    fn tools_dir(metadata: &PackageMetadata) -> Result<PathBuf, AdapterError> {
        crate::paths::setuppal_data_dir()
            .map(|d| d.join("tools").join(Self::repo_slug(metadata)))
            .map_err(|e| AdapterError::Other(e.to_string()))
    }

    fn asset_name(metadata: &PackageMetadata) -> Option<String> {
        metadata.download_url.as_deref().and_then(file_name_from_url)
    }

    /// Finds the folder holding the extracted executables (the root or one level down).
    fn find_exe_dir(root: &Path) -> Option<PathBuf> {
        let has_exe = |dir: &Path| {
            std::fs::read_dir(dir).ok().is_some_and(|rd| {
                rd.flatten().any(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|x| x.eq_ignore_ascii_case("exe"))
                })
            })
        };
        if has_exe(root) {
            return Some(root.to_path_buf());
        }
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        dirs.into_iter().find(|d| has_exe(d))
    }

    async fn download_release_asset(
        &self,
        metadata: &PackageMetadata,
        ctx: &ExecutionContext,
    ) -> Result<FetchedArtifact, AdapterError> {
        if !metadata.has_upstream_digest() {
            return Err(AdapterError::Other(format!(
                "GitHub published no checksum for the {} release asset; refusing an unverified download",
                metadata.package_id
            )));
        }
        let name = Self::asset_name(metadata)
            .ok_or_else(|| AdapterError::Other("release asset has no usable file name".to_string()))?;
        let dest = crate::paths::setuppal_data_dir()
            .map_err(|e| AdapterError::Other(e.to_string()))?
            .join("downloads")
            .join(format!(
                "{}-{}",
                metadata.package_id.replace('/', "-"),
                metadata.resolved_version
            ))
            .join(name);
        fetch_with_digests(&self.http, metadata, &dest, ctx).await
    }

    async fn run_checked(&self, binary: &str, args: Vec<String>, ok_codes: &[i32]) -> Result<(), AdapterError> {
        let paths = self
            .os_provider
            .refresh_and_get_path()
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;
        let bin = self
            .os_provider
            .which_in_paths(binary, &paths)
            .ok_or_else(|| AdapterError::Other(format!("{binary} executable not found on host")))?;
        let out = self
            .os_provider
            .run_command(&bin, &args, &paths)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;
        match out.exit_code {
            Some(code) if ok_codes.contains(&code) => Ok(()),
            _ => Err(AdapterError::CommandFailed {
                exit_code: out.exit_code,
                stdout: out.stdout,
                stderr: out.stderr,
            }),
        }
    }
}

#[async_trait]
impl SourceAdapter for GitHubReleasesAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::GitHubReleases
    }

    async fn resolve_metadata(
        &self,
        package_id: &str,
        version: Option<&str>,
    ) -> Result<PackageMetadata, AdapterError> {
        let (owner, repo) = Self::normalize_repo_id(package_id).ok_or_else(|| {
            AdapterError::Other(format!(
                "Invalid GitHub repository identifier `{package_id}` (expected 'owner/repo')"
            ))
        })?;

        let url = format!("{}/repos/{}/{}/releases", self.base_url, owner, repo);
        let response = self.http.get(&url).send().await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AdapterError::PackageNotFound {
                adapter: AdapterKind::GitHubReleases,
                package_id: package_id.to_string(),
            });
        }
        let releases: Vec<GitHubRelease> = response.error_for_status()?.json().await?;

        let release = match version {
            Some(ver) if !ver.is_empty() && ver != "latest" => releases
                .iter()
                .find(|r| r.tag_name == ver || r.tag_name.trim_start_matches('v') == ver.trim_start_matches('v')),
            _ => releases.iter().find(|r| !r.draft && !r.prerelease),
        }
        .ok_or_else(|| AdapterError::PackageNotFound {
            adapter: AdapterKind::GitHubReleases,
            package_id: package_id.to_string(),
        })?;

        let asset = Self::select_best_windows_asset(&release.assets);
        let mut meta = PackageMetadata::bare(
            AdapterKind::GitHubReleases,
            format!("{owner}/{repo}"),
            release.tag_name.clone(),
        );
        meta.published_at = parse_rfc3339(release.published_at.as_deref());
        meta.age_days = age_days_since(release.published_at.as_deref());
        meta.download_count = asset.as_ref().map(|a| a.download_count);
        meta.download_url = asset.as_ref().map(|a| a.browser_download_url.clone());
        meta.expected_sha256 = asset
            .as_ref()
            .and_then(|a| a.digest.as_deref())
            .and_then(|d| d.strip_prefix("sha256:"))
            .map(str::to_ascii_lowercase)
            .filter(|d| crate::safety::policy::is_valid_sha256_hex(d));
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
        artifact: Option<&FetchedArtifact>,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionReceipt, AdapterError> {
        let asset_name = Self::asset_name(metadata);
        let method = choose_install_method(&metadata.package_id, asset_name.as_deref());

        if let GitHubInstallMethod::NoUnattendedPath(reason) = &method {
            return Err(AdapterError::Other(format!("no unattended install path: {reason}")));
        }

        if let GitHubInstallMethod::Winget(id) = method {
            let winget = WingetAdapter::new(self.os_provider.clone());
            let winget_meta = if ctx.dry_run {
                PackageMetadata::bare(AdapterKind::Winget, id, "latest")
            } else {
                winget.resolve_metadata(id, None).await?
            };
            let mut receipt = winget.install(&winget_meta, None, ctx).await?;
            receipt.changes_made.insert(
                0,
                format!("Used winget package {id} instead of the raw {} GitHub release", metadata.package_id),
            );
            return Ok(receipt);
        }

        let tools_dir = Self::tools_dir(metadata)?;
        if ctx.dry_run {
            let action = match method {
                GitHubInstallMethod::Msi => "run msiexec /i <asset> /qn /norestart".to_string(),
                _ => format!("unpack into {} and add it to the User PATH", tools_dir.display()),
            };
            return Ok(ExecutionReceipt::new(
                AdapterKind::GitHubReleases,
                metadata.package_id.clone(),
                metadata.resolved_version.clone(),
                true,
                vec![format!(
                    "[Dry Run] Would download {} {} and {action}",
                    metadata.package_id, metadata.resolved_version
                )],
                None,
            ));
        }

        let fetched;
        let artifact = match artifact {
            Some(a) => a,
            None => {
                fetched = self.download_release_asset(metadata, ctx).await?;
                &fetched
            }
        };
        let file = artifact.local_path.clone();
        let file_str = file.display().to_string();

        match method {
            GitHubInstallMethod::Msi => {
                let args = ["/i", &file_str, "/qn", "/norestart", "ALLUSERS=2", "MSIINSTALLPERUSER=1"]
                    .map(String::from)
                    .to_vec();
                self.run_checked("msiexec", args, &[0, 3010]).await?;
                Ok(ExecutionReceipt::new(
                    AdapterKind::GitHubReleases,
                    metadata.package_id.clone(),
                    metadata.resolved_version.clone(),
                    false,
                    vec![format!("Installed {} {} with msiexec", metadata.package_id, metadata.resolved_version)],
                    Some(format!("msiexec /x {file_str} /qn /norestart")),
                )
                .with_uninstall_argv(
                    ["msiexec", "/x", &file_str, "/qn", "/norestart"].map(String::from).to_vec(),
                ))
            }
            GitHubInstallMethod::Zip | GitHubInstallMethod::PortableExe => {
                std::fs::create_dir_all(&tools_dir)?;
                let exe_dir = if method == GitHubInstallMethod::Zip {
                    let args = vec![
                        "-xf".to_string(),
                        file_str.clone(),
                        "-C".to_string(),
                        tools_dir.display().to_string(),
                    ];
                    if let Err(e) = self.run_checked("tar", args, &[0]).await {
                        let _ = std::fs::remove_dir_all(&tools_dir);
                        return Err(e);
                    }
                    Self::find_exe_dir(&tools_dir).ok_or_else(|| {
                        let _ = std::fs::remove_dir_all(&tools_dir);
                        AdapterError::Other(format!("{} contains no executable", file_str))
                    })?
                } else {
                    let target = tools_dir.join(format!("{}.exe", Self::repo_slug(metadata)));
                    std::fs::copy(&file, &target)?;
                    tools_dir.clone()
                };

                let mut receipt = ExecutionReceipt::new(
                    AdapterKind::GitHubReleases,
                    metadata.package_id.clone(),
                    metadata.resolved_version.clone(),
                    false,
                    vec![format!(
                        "Unpacked {} {} into {}",
                        metadata.package_id,
                        metadata.resolved_version,
                        tools_dir.display()
                    )],
                    Some(format!("delete {}", tools_dir.display())),
                );
                receipt.created_paths.push(tools_dir);
                receipt.path_entries.push(exe_dir);
                Ok(receipt)
            }
            GitHubInstallMethod::Winget(_) | GitHubInstallMethod::NoUnattendedPath(_) => {
                Err(AdapterError::Other("no unattended install path".to_string()))
            }
        }
    }

    async fn verify(
        &self,
        metadata: &PackageMetadata,
        ctx: &ExecutionContext,
    ) -> Result<VerificationResult, AdapterError> {
        let method = choose_install_method(&metadata.package_id, Self::asset_name(metadata).as_deref());
        match method {
            GitHubInstallMethod::Winget(id) => {
                let winget = WingetAdapter::new(self.os_provider.clone());
                winget
                    .verify(&PackageMetadata::bare(AdapterKind::Winget, id, "latest"), ctx)
                    .await
            }
            GitHubInstallMethod::Zip | GitHubInstallMethod::PortableExe => {
                let dir = Self::tools_dir(metadata)?;
                let found = Self::find_exe_dir(&dir);
                Ok(VerificationResult {
                    passed: found.is_some(),
                    command_run: format!("verify_tool_dir {}", dir.display()),
                    exit_code: Some(if found.is_some() { 0 } else { 1 }),
                    stdout: found
                        .map(|d| format!("Executables present in {}", d.display()))
                        .unwrap_or_default(),
                    stderr: String::new(),
                })
            }
            GitHubInstallMethod::Msi => Ok(VerificationResult::failed(
                format!("verify_msi {}", metadata.package_id),
                "No CLI check is known for this MSI; confirm the app in Settings > Apps",
            )),
            GitHubInstallMethod::NoUnattendedPath(reason) => {
                Ok(VerificationResult::failed(format!("verify {}", metadata.package_id), reason))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::mock::MockOsProvider;

    #[test]
    fn known_projects_prefer_winget() {
        assert_eq!(
            choose_install_method("BurntSushi/ripgrep", Some("ripgrep-14.1.0-x86_64-pc-windows-msvc.zip")),
            GitHubInstallMethod::Winget("BurntSushi.ripgrep.MSVC")
        );
    }

    #[test]
    fn asset_types_map_to_methods() {
        assert_eq!(choose_install_method("a/b", Some("b-1.0-x64.msi")), GitHubInstallMethod::Msi);
        assert_eq!(choose_install_method("a/b", Some("b-win64.zip")), GitHubInstallMethod::Zip);
        assert_eq!(choose_install_method("a/b", Some("b-windows-amd64.exe")), GitHubInstallMethod::PortableExe);
        assert!(matches!(
            choose_install_method("a/b", Some("b-Setup-1.0.exe")),
            GitHubInstallMethod::NoUnattendedPath(_)
        ));
        assert!(matches!(choose_install_method("a/b", None), GitHubInstallMethod::NoUnattendedPath(_)));
    }

    #[tokio::test]
    async fn install_without_unattended_path_fails_even_in_dry_run() {
        let adapter = GitHubReleasesAdapter::new(Arc::new(MockOsProvider::new()));
        let mut meta = PackageMetadata::bare(AdapterKind::GitHubReleases, "acme/widget", "v1.0");
        meta.download_url = Some("https://github.com/acme/widget/releases/download/v1.0/WidgetSetup.exe".to_string());
        for dry_run in [true, false] {
            let ctx = ExecutionContext::new(dry_run, ".".into(), None);
            let err = adapter.install(&meta, None, &ctx).await.unwrap_err();
            assert!(err.to_string().contains("no unattended install path"));
        }
    }

    #[tokio::test]
    async fn live_install_refuses_assets_without_checksum() {
        let adapter = GitHubReleasesAdapter::new(Arc::new(MockOsProvider::new()));
        let mut meta = PackageMetadata::bare(AdapterKind::GitHubReleases, "acme/widget", "v1.0");
        meta.download_url = Some("https://github.com/acme/widget/releases/download/v1.0/widget-win64.zip".to_string());
        let ctx = ExecutionContext::new(false, ".".into(), None,
        );
        let err = adapter.install(&meta, None, &ctx).await.unwrap_err();
        assert!(err.to_string().contains("no checksum"));
    }
}
