use std::path::PathBuf;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A structured, zero-trust execution plan generated from a deterministic recipe
/// or proposed by NVIDIA NIM. Never contains raw shell commands.
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
        destination: PathBuf,
        expected_sha256: String,
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
