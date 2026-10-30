use std::path::PathBuf;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A structured, zero-trust execution plan generated from a deterministic recipe
/// or proposed by a Nebius Nemotron model. Never contains raw shell commands.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Plan {
    pub plan_id: String,
    pub goal: String,
    pub summary: String,
    pub source: PlanSource,
    pub steps: Vec<PlannedStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanSource {
    Recipe { recipe_id: String },
    Llm { model: String },
    Healer { failed_step_id: String, attempt: u8 },
    /// One exact winget match for an app name the user asked for (see `resolver`).
    Resolved { query: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct PlannedStep {
    /// Deterministic identifier within the plan (e.g., "step-1")
    pub id: String,
    /// Human-readable explanation of why this step is needed
    pub description: String,
    /// The strictly typed action to execute
    pub step: Step,
}

/// Closed set of allowed actions. Serializes with explicit action tag and params
/// so the LLM cannot invent arbitrary shell executions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "action", content = "params", rename_all = "snake_case")]
pub enum Step {
    InstallPackage {
        ecosystem: Ecosystem,
        package_id: String,
        version: Option<String>,
        source_adapter: AdapterKind,
    },
    DownloadArtifact {
        adapter: AdapterKind,
        id: String,
        version: String,
        /// Relative paths resolve against the execution working directory. A leading
        /// `%APPDATA%`, `%LOCALAPPDATA%`, or `%USERPROFILE%` token expands to that folder.
        destination: PathBuf,
        /// Real upstream digest pinned by the recipe, or `None` when no checksum is known.
        /// When present it takes precedence over adapter metadata and is enforced after download.
        #[serde(default)]
        expected_sha256: Option<String>,
    },
    VerifyHash {
        file_path: PathBuf,
        expected_sha256: String,
    },
    AddToUserPath {
        directory: PathBuf,
    },
    CreateIsolatedEnv {
        kind: IsolationKind,
        path: PathBuf,
    },
    RunVerifyCommand {
        binary: AllowedBinary,
        args: Vec<String>,
    },
    /// Runs a known vendor installer headlessly with a fixed, typed argument set.
    RunVendorInstaller {
        installer: VendorInstaller,
        mc_version: String,
        /// Installer file downloaded by an earlier `DownloadArtifact` step (same path rules).
        installer_path: PathBuf,
    },
    /// Locates an installed tool on disk (winget install locations, vendor registry keys)
    /// and adds its directory to the User PATH.
    AddDiscoveredToolToPath {
        binary: AllowedBinary,
    },
}

/// Closed set of vendor installers SetupPal knows how to run unattended.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum VendorInstaller {
    /// Fabric mod loader for Minecraft Java Edition, installed into the existing
    /// launcher data folder: `java -jar <jar> client -dir <.minecraft> -mcversion <v> -noprofile`.
    FabricClient,
}

impl Step {
    /// Returns the transparent source origin: `(domain_or_label, optional_clickable_url)`
    /// As mandated by SECURITY.md: "The displayed domain must be the actual domain the download uses".
    pub fn source_display(&self) -> (String, Option<String>) {
        match self {
            Self::InstallPackage {
                ecosystem,
                package_id,
                ..
            } => match ecosystem {
                Ecosystem::PyPI => (
                    "pypi.org".to_string(),
                    Some(format!("https://pypi.org/project/{package_id}/")),
                ),
                Ecosystem::Npm => (
                    "registry.npmjs.org".to_string(),
                    Some(format!("https://www.npmjs.com/package/{package_id}")),
                ),
                Ecosystem::Modrinth => (
                    "api.modrinth.com".to_string(),
                    Some(format!("https://modrinth.com/project/{package_id}")),
                ),
                Ecosystem::Winget => (
                    "winget (Microsoft Community Repo)".to_string(),
                    Some("https://github.com/microsoft/winget-pkgs".to_string()),
                ),
                Ecosystem::GitHub => (
                    "github.com".to_string(),
                    Some(format!("https://github.com/{package_id}")),
                ),
                Ecosystem::Cargo => (
                    "crates.io".to_string(),
                    Some(format!("https://crates.io/crates/{package_id}")),
                ),
                Ecosystem::Vendor => ("Official Vendor Domain".to_string(), None),
            },
            Self::DownloadArtifact { id, .. } => {
                if id.starts_with("http://") || id.starts_with("https://") {
                    if let Ok(parsed) = url::Url::parse(id) {
                        if let Some(host) = parsed.host_str() {
                            return (host.to_string(), Some(id.clone()));
                        }
                    }
                    (id.clone(), Some(id.clone()))
                } else {
                    ("Local / Embedded Source".to_string(), None)
                }
            }
            Self::VerifyHash { .. } => ("Local Hash Check".to_string(), None),
            Self::AddToUserPath { .. } => ("Windows Registry (HKCU)".to_string(), None),
            Self::CreateIsolatedEnv { .. } => ("Local Isolated Directory".to_string(), None),
            Self::RunVerifyCommand { .. } => ("Local Direct Process Execution".to_string(), None),
            Self::RunVendorInstaller { installer, .. } => match installer {
                VendorInstaller::FabricClient => (
                    "maven.fabricmc.net".to_string(),
                    Some("https://fabricmc.net/use/installer/".to_string()),
                ),
            },
            Self::AddDiscoveredToolToPath { .. } => ("Windows Registry (HKCU)".to_string(), None),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Ecosystem {
    Winget,
    PyPI,
    Npm,
    Modrinth,
    GitHub,
    Vendor,
    Cargo,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    Winget,
    PyPI,
    Npm,
    Modrinth,
    GitHubReleases,
    OfficialVendorUrl,
    Cargo,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IsolationKind {
    PythonVenv,
    IsolatedDirectory,
}

/// Strictly allowlisted binaries for verification commands and system scanning.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AllowedBinary {
    Gpp,
    Clang,
    Cmake,
    Python,
    Pip,
    Node,
    Npm,
    Java,
    Git,
    Winget,
    Cargo,
    Rustc,
    Ninja,
}

impl AllowedBinary {
    pub const ALL: &'static [AllowedBinary] = &[
        Self::Git,
        Self::Python,
        Self::Pip,
        Self::Node,
        Self::Npm,
        Self::Cargo,
        Self::Rustc,
        Self::Java,
        Self::Winget,
        Self::Cmake,
        Self::Gpp,
        Self::Clang,
        Self::Ninja,
    ];

    pub const fn executable_name(self) -> &'static str {
        match self {
            Self::Gpp => "g++",
            Self::Clang => "clang++",
            Self::Cmake => "cmake",
            Self::Python => "python",
            Self::Pip => "pip",
            Self::Node => "node",
            Self::Npm => "npm",
            Self::Java => "java",
            Self::Git => "git",
            Self::Winget => "winget",
            Self::Cargo => "cargo",
            Self::Rustc => "rustc",
            Self::Ninja => "ninja",
        }
    }

    pub const fn version_flag(self) -> &'static str {
        match self {
            Self::Java => "-version",
            _ => "--version",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SystemProfile {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub gpu_detected: Option<String>,
    pub free_disk_bytes: u64,
    pub installed_tools: Vec<DetectedTool>,
    pub active_path_entries: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DetectedTool {
    pub binary: AllowedBinary,
    pub version_output: String,
    pub path: PathBuf,
}
