//! `installer_core` — Zero-Trust AI Environment & Package Orchestrator core library.
//!
//! Shares 100% of domain logic across the CLI (`installer_cli`) and Tauri v2 GUI (`installer_gui`).

pub mod adapters;
pub mod alternatives;
pub mod cleanup;
pub mod config;
pub mod discovery;
pub mod events;
pub mod executor;
pub mod healer;
pub mod installability;
pub mod llm;
pub mod os;
pub mod paths;
pub mod plan_scope;
pub mod planner;
pub mod recipes;
pub mod resolver;
pub mod safety;
pub mod scan;
pub mod state;
pub mod types;
pub mod verifier;

pub use adapters::{
    AdapterError, AdapterRegistry, CargoAdapter, ExecutionContext, ExecutionReceipt,
    FetchedArtifact, GitHubReleasesAdapter, ModrinthAdapter, NpmAdapter, PackageMetadata,
    PyPIAdapter, SourceAdapter, VendorUrlAdapter, VerificationResult, WingetAdapter,
};
pub use alternatives::{
    alternatives_for, detect_category, filter_same_category, is_broad_request, Alternative,
    AppCategory,
};
pub use installability::{
    apply_to_safety_report, check_plan, first_manual, manual_guide, manual_guide_with_installed,
    Installability, ManualGuide, ManualReason, StepInstallability,
};
pub use config::{fixed_secret, CoreConfig, CoreError, NemotronModel, SecretProvider};
pub use events::{AgentEvent, EventBus};
pub use executor::{
    ExecutionOptions, Executor, PlanExecutionSummary, StepExecutionResult, StepRunStatus,
    TrustCategory,
};
pub use healer::{FailureContext, Healer};
pub use llm::{LlmClient, ModelSelection, NimClient, ProviderClient};
pub use os::{CommandOutput, OsProvider, PlatformOsProvider};
pub use planner::Planner;
pub use recipes::{Recipe, RecipeStore};
pub use safety::{
    HttpTavilyVerifier, PlanSafetyReport, RiskRating, SafetyEngine,
    SafetyReport, SafetySignals, TavilyVerificationResult, TavilyVerifier, TyposquatMatch,
    VulnerabilitySummary,
};
pub use scan::SystemScanner;
pub use state::{RollbackData, StateLogger, TransactionRecord};
pub use types::{
    AdapterKind, AllowedBinary, DetectedTool, Ecosystem, IsolationKind, Plan, PlanSource,
    PlannedStep, Step, SystemProfile, VendorInstaller,
};
pub use verifier::Verifier;
