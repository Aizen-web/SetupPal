//! `installer_core` — Zero-Trust AI Environment & Package Orchestrator core library.
//!
//! Shares 100% of domain logic across the CLI (`installer_cli`) and Tauri v2 GUI (`installer_gui`).

pub mod adapters;
pub mod config;
pub mod events;
pub mod llm;
pub mod os;
pub mod planner;
pub mod recipes;
pub mod safety;
pub mod scan;
pub mod types;

pub use adapters::{
    AdapterError, AdapterRegistry, ExecutionContext, ExecutionReceipt, FetchedArtifact,
    PackageMetadata, SourceAdapter, VerificationResult,
};
pub use config::{CoreConfig, CoreError};
pub use events::{AgentEvent, EventBus};
pub use llm::{LlmClient, NimClient};
pub use os::{CommandOutput, OsProvider, PlatformOsProvider};
pub use planner::Planner;
pub use recipes::{Recipe, RecipeStore};
pub use safety::{
    PlanSafetyReport, RiskRating, SafetyEngine, SafetyReport, SafetySignals, TyposquatMatch,
    VulnerabilitySummary,
};
pub use scan::SystemScanner;
pub use types::{
    AdapterKind, AllowedBinary, DetectedTool, Ecosystem, IsolationKind, Plan, PlanSource,
    PlannedStep, Step, SystemProfile,
};
