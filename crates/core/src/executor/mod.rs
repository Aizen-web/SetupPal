use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::adapters::pypi::venv_python;
use crate::adapters::{
    AdapterError, AdapterRegistry, ExecutionContext, ExecutionReceipt, SourceAdapter,
    VerificationResult,
};
use crate::config::CoreError;
use crate::events::{AgentEvent, EventBus};
use crate::os::OsProvider;
use crate::paths::{minecraft_dir, resolve_step_path};
use crate::safety::hash::verify_file_sha256;
use crate::safety::{PlanSafetyReport, RiskRating};
use crate::state::{RollbackData, StateLogger, TransactionRecord};
use crate::types::{AdapterKind, AllowedBinary, IsolationKind, Plan, PlannedStep, Step, VendorInstaller};
use crate::verifier::{verify_fabric_profile, Verifier};

/// Categories of software installs that can be narrowly trusted per SECURITY.md.
/// "An optional 'don't ask me about this kind of thing again' checkbox,
/// scoped to a category like 'verified-publisher winget installs', never 'everything'."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustCategory {
    VerifiedWinget,
    IsolatedPythonVenv,
    LocalNpmModules,
    OfficialGitHubRelease,
}

#[derive(Debug, Clone)]
pub struct ExecutionOptions {
    pub dry_run: bool,
    /// Runs Green steps without asking. When false, a live run halts at the first Green
    /// step that is neither in `approved_step_ids` nor a trusted category.
    pub auto_approve_green: bool,
    /// Green step IDs the user explicitly approved for this run.
    pub approved_step_ids: HashSet<String>,
    /// Yellow step IDs the user explicitly approved for this run.
    pub allow_yellow_steps: HashSet<String>,
    /// Skip steps before this ID (already recorded); used after approving a held step.
    pub start_at_step_id: Option<String>,
    /// Base folder for relative step paths and npm installs (the user's project folder).
    pub working_dir: PathBuf,
    pub trusted_categories: HashSet<TrustCategory>,
    /// Cancels the run between steps and aborts in-flight OS commands.
    pub cancel: CancellationToken,
    /// Client-generated run id; included in `ExecutionCancelled` events.
    pub run_id: String,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            dry_run: true,
            auto_approve_green: false,
            approved_step_ids: HashSet::new(),
            allow_yellow_steps: HashSet::new(),
            start_at_step_id: None,
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            trusted_categories: HashSet::new(),
            cancel: CancellationToken::new(),
            run_id: String::new(),
        }
    }
}

impl ExecutionOptions {
    /// Checks whether a step belongs to a narrowly trusted category.
    /// As mandated by SECURITY.md, later Yellow and Red steps still interrupt regardless.
    pub fn is_step_trusted(&self, step: &Step) -> bool {
        match step {
            Step::InstallPackage {
                ecosystem: crate::types::Ecosystem::Winget,
                ..
            } => self.trusted_categories.contains(&TrustCategory::VerifiedWinget),
            Step::InstallPackage {
                ecosystem: crate::types::Ecosystem::Npm,
                ..
            } => self.trusted_categories.contains(&TrustCategory::LocalNpmModules),
            Step::CreateIsolatedEnv {
                kind: IsolationKind::PythonVenv,
                ..
            } => self.trusted_categories.contains(&TrustCategory::IsolatedPythonVenv),
            Step::DownloadArtifact {
                adapter: AdapterKind::GitHubReleases,
                ..
            } => self.trusted_categories.contains(&TrustCategory::OfficialGitHubRelease),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepRunStatus {
    Done,
    Failed,
    Blocked,
    Held,
    Skipped,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanExecutionSummary {
    pub plan_id: String,
    pub goal: String,
    pub dry_run: bool,
    pub step_results: Vec<StepExecutionResult>,
    pub overall_passed: bool,
    pub toolchain_verification: Option<VerificationResult>,
    /// Step the run stopped at because it needs user approval (Green without
    /// auto-approve, or Yellow not in `allow_yellow_steps`).
    #[serde(default)]
    pub halted_at_step_id: Option<String>,
    /// True when the run stopped because the cancel token fired.
    #[serde(default)]
    pub cancelled: bool,
}

impl PlanExecutionSummary {
    /// One line the UI shows above the results. Dry runs are always called out so nobody
    /// mistakes a simulation for a real install.
    pub fn headline(&self) -> String {
        let steps = self.step_results.len();
        let plural = if steps == 1 { "" } else { "s" };
        if self.dry_run {
            let outcome = if self.cancelled {
                "cancelled"
            } else if self.halted_at_step_id.is_some() {
                "paused for approval"
            } else if self.overall_passed {
                "finished"
            } else {
                "found a problem"
            };
            format!("DRY RUN {outcome}: {steps} step{plural} simulated. Nothing was installed or changed on this PC.")
        } else if self.cancelled {
            format!("Run cancelled after {steps} step{plural}.")
        } else if self.halted_at_step_id.is_some() {
            format!("Run paused for approval after {steps} step{plural}.")
        } else if self.overall_passed {
            format!("Run finished: {steps} step{plural} completed and verified.")
        } else {
            format!("Run failed after {steps} step{plural}.")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepExecutionResult {
    pub step_id: String,
    pub description: String,
    pub executed: bool,
    pub blocked_reason: Option<String>,
    pub receipt: Option<ExecutionReceipt>,
    pub verification: Option<VerificationResult>,
    /// Real error text (including installer stderr) when the step failed while running.
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default = "default_step_status")]
    pub status: StepRunStatus,
}

fn default_step_status() -> StepRunStatus {
    StepRunStatus::Done
}

impl StepExecutionResult {
    fn not_run(step: &PlannedStep, reason: String, status: StepRunStatus) -> Self {
        Self {
            step_id: step.id.clone(),
            description: step.description.clone(),
            executed: false,
            blocked_reason: Some(reason),
            receipt: None,
            verification: None,
            error: None,
            status,
        }
    }

    fn done(step: &PlannedStep, receipt: ExecutionReceipt, verification: Option<VerificationResult>) -> Self {
        Self {
            step_id: step.id.clone(),
            description: step.description.clone(),
            executed: true,
            blocked_reason: None,
            receipt: Some(receipt),
            verification,
            error: None,
            status: StepRunStatus::Done,
        }
    }

    /// True when the step ran, raised no error, and any verification passed.
    pub fn succeeded(&self) -> bool {
        self.executed
            && self.error.is_none()
            && self.status == StepRunStatus::Done
            && self.verification.as_ref().is_none_or(|v| v.passed)
    }
}

/// Mutable state threaded through the steps of one plan run.
#[derive(Debug, Default)]
struct RunState {
    /// venv created by an earlier `CreateIsolatedEnv` step; pip and python use it.
    isolated_env: Option<PathBuf>,
}

/// Zero-Trust Plan Executor.
/// Enforces hard safety gates (NEVER executes Red; requires confirmation for Yellow;
/// runs Green only when auto-approved or trusted; records every transaction to state log).
pub struct Executor {
    adapters: AdapterRegistry,
    os_provider: Arc<dyn OsProvider>,
    state_logger: StateLogger,
    verifier: Verifier,
    events: EventBus,
}

impl Executor {
    pub fn new(
        adapters: AdapterRegistry,
        os_provider: Arc<dyn OsProvider>,
        state_logger: StateLogger,
        verifier: Verifier,
        events: EventBus,
    ) -> Self {
        Self {
            adapters,
            os_provider,
            state_logger,
            verifier,
            events,
        }
    }

    /// Executes an evaluated Plan against safety policies.
    pub async fn execute_plan(
        &self,
        plan: &Plan,
        safety_report: &PlanSafetyReport,
        options: &ExecutionOptions,
    ) -> Result<PlanExecutionSummary, CoreError> {
        let mut step_results = Vec::new();
        let mut all_succeeded = true;
        let mut halted_at_step_id = None;
        let mut cancelled = false;
        let mut run = RunState::default();
        let mut started = options.start_at_step_id.is_none();

        for step in &plan.steps {
            if !started {
                if options.start_at_step_id.as_deref() == Some(step.id.as_str()) {
                    started = true;
                } else {
                    step_results.push(StepExecutionResult::not_run(
                        step,
                        "Skipped (already completed or waiting to resume)".to_string(),
                        StepRunStatus::Skipped,
                    ));
                    continue;
                }
            }

            if options.cancel.is_cancelled() {
                self.events.emit(AgentEvent::ExecutionCancelled {
                    run_id: options.run_id.clone(),
                    step_id: step.id.clone(),
                });
                step_results.push(StepExecutionResult::not_run(
                    step,
                    "Cancelled by user".to_string(),
                    StepRunStatus::Cancelled,
                ));
                all_succeeded = false;
                cancelled = true;
                break;
            }

            let step_safety = safety_report
                .step_reports
                .iter()
                .find(|r| r.step_id == step.id);
            let rating = step_safety.map(|s| s.rating).unwrap_or(RiskRating::Yellow);
            let reasons = step_safety.map(|s| s.reasons.clone()).unwrap_or_default();

            // 1. Hard Red Block — never runs, even if listed in approval sets.
            if rating == RiskRating::Red {
                let reason = format!(
                    "BLOCKED by Zero-Trust Safety Engine: Step '{}' scored RED risk. (Reasons: {})",
                    step.id,
                    if reasons.is_empty() {
                        "Critical threat detected".to_string()
                    } else {
                        reasons.join("; ")
                    }
                );
                self.events.emit(AgentEvent::StepExecutionBlocked {
                    step_id: step.id.clone(),
                    reason: reason.clone(),
                });
                step_results.push(StepExecutionResult::not_run(
                    step,
                    reason,
                    StepRunStatus::Blocked,
                ));
                all_succeeded = false;
                break;
            }

            // 2. Yellow Caution Gate — only when this step id was explicitly approved.
            if rating == RiskRating::Yellow && !options.allow_yellow_steps.contains(&step.id) {
                let reason = format!(
                    "Confirmation required: Step '{}' scored YELLOW risk. Approve this step to continue. Reasons: {}",
                    step.id,
                    reasons.join("; ")
                );
                self.events.emit(AgentEvent::StepApprovalRequired {
                    step_id: step.id.clone(),
                    rating: RiskRating::Yellow,
                    reasons,
                });
                step_results.push(StepExecutionResult::not_run(
                    step,
                    reason,
                    StepRunStatus::Held,
                ));
                halted_at_step_id = Some(step.id.clone());
                all_succeeded = false;
                break;
            }

            // 3. Green Approval Gate: live runs halt unless auto-approved, listed, or trusted.
            let green_ok = options.auto_approve_green
                || options.approved_step_ids.contains(&step.id)
                || options.is_step_trusted(&step.step);
            if rating == RiskRating::Green && !options.dry_run && !green_ok {
                self.events.emit(AgentEvent::StepApprovalRequired {
                    step_id: step.id.clone(),
                    rating: RiskRating::Green,
                    reasons: vec!["Awaiting your approval for this safe step".to_string()],
                });
                step_results.push(StepExecutionResult::not_run(
                    step,
                    format!(
                        "Awaiting approval: Step '{}' is safe but needs your go-ahead.",
                        step.id
                    ),
                    StepRunStatus::Held,
                ));
                halted_at_step_id = Some(step.id.clone());
                all_succeeded = false;
                break;
            }

            let mut step_result = match self.execute_single_step(plan, step, options, &mut run).await {
                Ok(result) => result,
                Err(e) => {
                    let msg = e.to_string();
                    let status = if options.cancel.is_cancelled()
                        || msg.to_ascii_lowercase().contains("cancelled")
                    {
                        StepRunStatus::Cancelled
                    } else {
                        StepRunStatus::Failed
                    };
                    StepExecutionResult {
                        step_id: step.id.clone(),
                        description: step.description.clone(),
                        executed: true,
                        blocked_reason: None,
                        receipt: None,
                        verification: None,
                        error: Some(msg),
                        status,
                    }
                }
            };

            if step_result.status == StepRunStatus::Cancelled || options.cancel.is_cancelled() {
                step_result.status = StepRunStatus::Cancelled;
                self.events.emit(AgentEvent::ExecutionCancelled {
                    run_id: options.run_id.clone(),
                    step_id: step.id.clone(),
                });
                step_results.push(step_result);
                all_succeeded = false;
                cancelled = true;
                break;
            }

            if let Some(err) = &step_result.error {
                step_result.status = StepRunStatus::Failed;
                self.events.emit(AgentEvent::StepExecutionFailed {
                    step_id: step.id.clone(),
                    error: err.clone(),
                });
            } else if let Some(v) = step_result.verification.as_ref().filter(|v| !v.passed) {
                step_result.status = StepRunStatus::Failed;
                step_result.error = Some(
                    format!("Verification failed: {} {}", v.command_run, v.stderr)
                        .trim()
                        .to_string(),
                );
                self.events.emit(AgentEvent::StepExecutionFailed {
                    step_id: step.id.clone(),
                    error: step_result.error.clone().unwrap_or_default(),
                });
            }

            let ok = step_result.succeeded();
            step_results.push(step_result);
            if !ok {
                all_succeeded = false;
                break;
            }
        }

        // 4. Goal-level post-install verification for C++ toolchain goals
        let goal_lower = plan.goal.to_ascii_lowercase();
        let is_cpp_goal = goal_lower.contains("c++") || goal_lower.contains("cpp");

        let toolchain_verification = if all_succeeded && is_cpp_goal && !options.dry_run {
            let res = self
                .verifier
                .verify_cpp_toolchain(&format!("{}-cpp-verify", plan.plan_id))
                .await?;
            if !res.passed {
                all_succeeded = false;
            }
            Some(res)
        } else {
            None
        };

        Ok(PlanExecutionSummary {
            plan_id: plan.plan_id.clone(),
            goal: plan.goal.clone(),
            dry_run: options.dry_run,
            step_results,
            overall_passed: all_succeeded && !cancelled,
            toolchain_verification,
            halted_at_step_id,
            cancelled,
        })
    }

    fn adapter_for(&self, kind: AdapterKind, step: &PlannedStep) -> Result<Arc<dyn SourceAdapter>, CoreError> {
        self.adapters.get(kind).ok_or_else(|| {
            CoreError::Validation(format!(
                "No adapter registered for {:?} required by step {}",
                kind, step.id
            ))
        })
    }

    fn record(
        &self,
        plan: &Plan,
        step: &PlannedStep,
        options: &ExecutionOptions,
        action_summary: String,
        receipt: &ExecutionReceipt,
        rollback_data: Option<RollbackData>,
    ) -> Result<(), CoreError> {
        self.state_logger.record_transaction(&TransactionRecord {
            tx_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            plan_id: plan.plan_id.clone(),
            goal: plan.goal.clone(),
            step_id: step.id.clone(),
            action_summary,
            step: step.step.clone(),
            dry_run: options.dry_run,
            changes_made: receipt.changes_made.clone(),
            rollback_hint: receipt.rollback_hint.clone(),
            rollback_data,
            rolled_back_tx_id: None,
        })
    }

    /// Adds `directory` to the User PATH and records a rollback transaction for it.
    async fn add_path_entry(
        &self,
        plan: &Plan,
        step: &PlannedStep,
        options: &ExecutionOptions,
        directory: &Path,
    ) -> Result<ExecutionReceipt, CoreError> {
        let receipt = ExecutionReceipt::new(
            AdapterKind::Winget,
            "PATH",
            "registry",
            options.dry_run,
            vec![if options.dry_run {
                format!("[Dry Run] Would register {} on User PATH", directory.display())
            } else {
                format!("Registered {} on User PATH and refreshed process environment", directory.display())
            }],
            Some(format!("Remove {} from PATH", directory.display())),
        );
        if options.dry_run {
            return Ok(receipt);
        }
        let previous_path = self.os_provider.add_to_user_path(directory).await?;
        self.os_provider.refresh_and_get_path().await?;
        self.record(
            plan,
            step,
            options,
            format!("Added {} to User PATH", directory.display()),
            &receipt,
            Some(RollbackData::PathBackup {
                previous_path,
                added_directory: directory.to_path_buf(),
            }),
        )?;
        Ok(receipt)
    }

    async fn execute_single_step(
        &self,
        plan: &Plan,
        step: &PlannedStep,
        options: &ExecutionOptions,
        run: &mut RunState,
    ) -> Result<StepExecutionResult, CoreError> {
        self.events.emit(AgentEvent::StepExecutionStarted {
            step_id: step.id.clone(),
            description: step.description.clone(),
            dry_run: options.dry_run,
        });

        let exec_ctx = ExecutionContext::new(
            options.dry_run,
            options.working_dir.clone(),
            run.isolated_env.clone(),
        )
        .with_cancel(options.cancel.clone());

        let result = match &step.step {
            Step::InstallPackage {
                ecosystem,
                package_id,
                version,
                source_adapter,
            } => {
                let adapter = self.adapter_for(*source_adapter, step)?;
                let metadata = adapter
                    .resolve_metadata(package_id, version.as_deref())
                    .await
                    .map_err(CoreError::Adapter)?;
                let mut receipt = adapter
                    .install(&metadata, None, &exec_ctx)
                    .await
                    .map_err(CoreError::Adapter)?;

                if !options.dry_run {
                    self.os_provider.refresh_and_get_path().await?;
                }

                let rollback_data = if !receipt.created_paths.is_empty() {
                    Some(RollbackData::CreatedPaths {
                        paths: receipt.created_paths.clone(),
                    })
                } else {
                    Some(RollbackData::PackageInstall {
                        ecosystem: format!("{ecosystem:?}"),
                        package_id: package_id.clone(),
                        uninstall_command: receipt.rollback_hint.clone(),
                        uninstall_argv: receipt.uninstall_argv.clone(),
                    })
                };
                self.record(
                    plan,
                    step,
                    options,
                    format!(
                        "Installed package {}@{} via {:?}",
                        package_id, metadata.resolved_version, source_adapter
                    ),
                    &receipt,
                    rollback_data,
                )?;

                for dir in receipt.path_entries.clone() {
                    let path_receipt = self.add_path_entry(plan, step, options, &dir).await?;
                    receipt.changes_made.extend(path_receipt.changes_made);
                }

                self.events.emit(AgentEvent::StepExecutionCompleted {
                    step_id: step.id.clone(),
                    receipt: receipt.clone(),
                });

                let verification = if options.dry_run {
                    None
                } else {
                    Some(
                        self.verifier
                            .verify_installed_package(*ecosystem, adapter.as_ref(), &metadata, &exec_ctx)
                            .await?,
                    )
                };
                StepExecutionResult::done(step, receipt, verification)
            }

            Step::AddToUserPath { directory } => {
                let directory = resolve_step_path(directory, &options.working_dir)?;
                let receipt = self.add_path_entry(plan, step, options, &directory).await?;
                self.events.emit(AgentEvent::StepExecutionCompleted {
                    step_id: step.id.clone(),
                    receipt: receipt.clone(),
                });
                let verification = if options.dry_run {
                    None
                } else {
                    Some(self.verifier.verify_directory_on_path(&step.id, &directory).await?)
                };
                StepExecutionResult::done(step, receipt, verification)
            }

            Step::AddDiscoveredToolToPath { binary } => {
                self.add_discovered_tool(plan, step, options, *binary).await?
            }

            Step::RunVerifyCommand { binary, args } => {
                let env = run.isolated_env.as_deref();
                let verification = if options.dry_run {
                    let paths = self.os_provider.refresh_and_get_path().await?;
                    let present = env.is_some_and(|e| venv_python(e).is_file())
                        || self
                            .os_provider
                            .which_in_paths(binary.executable_name(), &paths)
                            .is_some();
                    if present {
                        Some(
                            self.verifier
                                .verify_binary_command_in_env(&step.id, *binary, args, env)
                                .await?,
                        )
                    } else {
                        None
                    }
                } else {
                    Some(
                        self.verifier
                            .verify_binary_command_in_env(&step.id, *binary, args, env)
                            .await?,
                    )
                };

                let receipt = ExecutionReceipt::new(
                    AdapterKind::Winget,
                    binary.executable_name(),
                    "verify",
                    options.dry_run,
                    vec![match &verification {
                        Some(_) => format!(
                            "Executed verification check: {} {}",
                            binary.executable_name(),
                            args.join(" ")
                        ),
                        None => format!(
                            "[Dry Run] {} is not installed yet; check deferred to the live run",
                            binary.executable_name()
                        ),
                    }],
                    None,
                );
                self.events.emit(AgentEvent::StepExecutionCompleted {
                    step_id: step.id.clone(),
                    receipt: receipt.clone(),
                });
                StepExecutionResult::done(step, receipt, verification)
            }

            Step::VerifyHash {
                file_path,
                expected_sha256,
            } => {
                let file_path = resolve_step_path(file_path, &options.working_dir)?;
                let verification = self
                    .verifier
                    .verify_hash_step(&step.id, &file_path, expected_sha256)
                    .await?;
                let receipt = ExecutionReceipt::new(
                    AdapterKind::Winget,
                    file_path.to_string_lossy().to_string(),
                    "sha256",
                    options.dry_run,
                    vec![format!("Checked SHA-256 for {}", file_path.display())],
                    None,
                );
                self.events.emit(AgentEvent::StepExecutionCompleted {
                    step_id: step.id.clone(),
                    receipt: receipt.clone(),
                });
                StepExecutionResult::done(step, receipt, Some(verification))
            }

            Step::DownloadArtifact {
                adapter,
                id,
                version,
                destination,
                expected_sha256,
            } => {
                let destination = resolve_step_path(destination, &options.working_dir)?;
                let source_adapter = self.adapter_for(*adapter, step)?;
                let mut metadata = source_adapter
                    .resolve_metadata(id, Some(version))
                    .await
                    .map_err(CoreError::Adapter)?;
                if let Some(pinned) = expected_sha256 {
                    metadata.expected_sha256 = Some(pinned.trim().to_ascii_lowercase());
                }

                if options.dry_run {
                    let receipt = ExecutionReceipt::new(
                        *adapter,
                        id.clone(),
                        metadata.resolved_version.clone(),
                        true,
                        vec![format!(
                            "[Dry Run] Would download {} {} to {} and check its {}",
                            id,
                            metadata.resolved_version,
                            destination.display(),
                            if metadata.has_upstream_digest() {
                                "official checksum"
                            } else {
                                "checksum (none published: the live run will refuse it)"
                            }
                        )],
                        Some(format!("delete {}", destination.display())),
                    );
                    self.events.emit(AgentEvent::StepExecutionCompleted {
                        step_id: step.id.clone(),
                        receipt: receipt.clone(),
                    });
                    return Ok(StepExecutionResult::done(step, receipt, None));
                }

                if !metadata.has_upstream_digest() {
                    return Err(CoreError::Adapter(AdapterError::Other(format!(
                        "No upstream checksum available for {id}; refusing to accept an unverified download"
                    ))));
                }

                let artifact = source_adapter
                    .fetch_artifact(&metadata, &destination, &exec_ctx)
                    .await
                    .map_err(CoreError::Adapter)?;

                let verification = enforce_download_hash(
                    &artifact.local_path,
                    metadata.expected_sha256.as_deref(),
                    artifact.computed_sha256.as_deref(),
                )?;

                let receipt = ExecutionReceipt::new(
                    *adapter,
                    id.clone(),
                    metadata.resolved_version.clone(),
                    false,
                    vec![format!(
                        "Downloaded {} {} to {} ({})",
                        id,
                        metadata.resolved_version,
                        artifact.local_path.display(),
                        verification.stdout
                    )],
                    Some(format!("delete {}", artifact.local_path.display())),
                );
                self.record(
                    plan,
                    step,
                    options,
                    format!("Downloaded artifact {}", artifact.local_path.display()),
                    &receipt,
                    Some(RollbackData::FileDownload {
                        destination: artifact.local_path.clone(),
                    }),
                )?;
                self.events.emit(AgentEvent::StepExecutionCompleted {
                    step_id: step.id.clone(),
                    receipt: receipt.clone(),
                });
                StepExecutionResult::done(step, receipt, Some(verification))
            }

            Step::CreateIsolatedEnv { kind, path } => {
                let path = resolve_step_path(path, &options.working_dir)?;
                self.create_isolated_env(plan, step, options, *kind, &path, run).await?
            }

            Step::RunVendorInstaller {
                installer,
                mc_version,
                installer_path,
            } => {
                let installer_path = resolve_step_path(installer_path, &options.working_dir)?;
                match installer {
                    VendorInstaller::FabricClient => {
                        self.run_fabric_installer(plan, step, options, mc_version, &installer_path)
                            .await?
                    }
                }
            }
        };

        Ok(result)
    }

    async fn create_isolated_env(
        &self,
        plan: &Plan,
        step: &PlannedStep,
        options: &ExecutionOptions,
        kind: IsolationKind,
        path: &Path,
        run: &mut RunState,
    ) -> Result<StepExecutionResult, CoreError> {
        let rollback_hint = Some(format!("rmdir {}", path.display()));

        if kind == IsolationKind::IsolatedDirectory {
            let existed = path.exists();
            if !options.dry_run && !existed {
                std::fs::create_dir_all(path)?;
            }
            let receipt = ExecutionReceipt::new(
                AdapterKind::OfficialVendorUrl,
                "IsolatedDirectory",
                "1",
                options.dry_run,
                vec![format!("Prepared isolated directory {}", path.display())],
                rollback_hint,
            );
            if !options.dry_run && !existed {
                self.record(
                    plan,
                    step,
                    options,
                    format!("Created isolated directory {}", path.display()),
                    &receipt,
                    Some(RollbackData::IsolatedEnv { path: path.to_path_buf() }),
                )?;
            }
            self.events.emit(AgentEvent::StepExecutionCompleted {
                step_id: step.id.clone(),
                receipt: receipt.clone(),
            });
            return Ok(StepExecutionResult::done(step, receipt, None));
        }

        run.isolated_env = Some(path.to_path_buf());
        if options.dry_run {
            let receipt = ExecutionReceipt::new(
                AdapterKind::PyPI,
                "PythonVenv",
                "venv",
                true,
                vec![format!("[Dry Run] Would run: python -m venv {}", path.display())],
                rollback_hint,
            );
            self.events.emit(AgentEvent::StepExecutionCompleted {
                step_id: step.id.clone(),
                receipt: receipt.clone(),
            });
            return Ok(StepExecutionResult::done(step, receipt, None));
        }

        let already_present = venv_python(path).is_file();
        let mut changes = Vec::new();
        if already_present {
            changes.push(format!("Reusing existing venv at {}", path.display()));
        } else {
            let paths = self.os_provider.refresh_and_get_path().await?;
            let python = self
                .os_provider
                .which_in_paths(AllowedBinary::Python.executable_name(), &paths)
                .or_else(|| self.os_provider.which_in_paths("python3", &paths))
                .ok_or_else(|| CoreError::Os("Python executable not found; install Python first".to_string()))?;
            let args = vec![
                "-m".to_string(),
                "venv".to_string(),
                path.to_string_lossy().to_string(),
            ];
            let out = self
                .os_provider
                .run_command_cancellable(&python, &args, &paths, None, Some(&options.cancel))
                .await?;
            if out.exit_code != Some(0) {
                return Err(CoreError::Adapter(AdapterError::CommandFailed {
                    exit_code: out.exit_code,
                    stdout: out.stdout,
                    stderr: out.stderr,
                }));
            }
            changes.push(format!("Created Python venv at {} with {}", path.display(), python.display()));
        }

        let receipt = ExecutionReceipt::new(
            AdapterKind::PyPI,
            "PythonVenv",
            "venv",
            false,
            changes,
            rollback_hint,
        );
        if !already_present {
            self.record(
                plan,
                step,
                options,
                format!("Created isolated env at {}", path.display()),
                &receipt,
                Some(RollbackData::IsolatedEnv { path: path.to_path_buf() }),
            )?;
        }
        self.events.emit(AgentEvent::StepExecutionCompleted {
            step_id: step.id.clone(),
            receipt: receipt.clone(),
        });
        let verification = self.verifier.verify_venv(path).await?;
        Ok(StepExecutionResult::done(step, receipt, Some(verification)))
    }

    async fn run_fabric_installer(
        &self,
        plan: &Plan,
        step: &PlannedStep,
        options: &ExecutionOptions,
        mc_version: &str,
        installer_path: &Path,
    ) -> Result<StepExecutionResult, CoreError> {
        let mc_dir = minecraft_dir()
            .ok_or_else(|| CoreError::Config("cannot locate the Minecraft data folder".to_string()))?;
        let args = vec![
            "-jar".to_string(),
            installer_path.to_string_lossy().to_string(),
            "client".to_string(),
            "-dir".to_string(),
            mc_dir.to_string_lossy().to_string(),
            "-mcversion".to_string(),
            mc_version.to_string(),
            "-noprofile".to_string(),
        ];

        if options.dry_run {
            let receipt = ExecutionReceipt::new(
                AdapterKind::OfficialVendorUrl,
                "fabric-installer",
                mc_version,
                true,
                vec![format!("[Dry Run] Would run: java {}", args.join(" "))],
                None,
            );
            self.events.emit(AgentEvent::StepExecutionCompleted {
                step_id: step.id.clone(),
                receipt: receipt.clone(),
            });
            return Ok(StepExecutionResult::done(step, receipt, None));
        }

        if !mc_dir.is_dir() {
            return Err(CoreError::Validation(format!(
                "Minecraft Java Edition is not set up at {}. Open the Minecraft Launcher and play {mc_version} once, then retry.",
                mc_dir.display()
            )));
        }
        if !installer_path.is_file() {
            return Err(CoreError::Validation(format!(
                "Fabric installer {} is missing; the download step must run first",
                installer_path.display()
            )));
        }

        let paths = self.os_provider.refresh_and_get_path().await?;
        let java = self
            .os_provider
            .which_in_paths(AllowedBinary::Java.executable_name(), &paths)
            .ok_or_else(|| CoreError::Os("java executable not found; install a Java runtime first".to_string()))?;

        let versions_dir = mc_dir.join("versions");
        let before = list_dir_names(&versions_dir);
        let out = self
            .os_provider
            .run_command_cancellable(&java, &args, &paths, Some(&mc_dir), Some(&options.cancel))
            .await?;
        if out.exit_code != Some(0) {
            return Err(CoreError::Adapter(AdapterError::CommandFailed {
                exit_code: out.exit_code,
                stdout: out.stdout,
                stderr: out.stderr,
            }));
        }
        let created: Vec<PathBuf> = list_dir_names(&versions_dir)
            .difference(&before)
            .map(|n| versions_dir.join(n))
            .collect();

        let mut receipt = ExecutionReceipt::new(
            AdapterKind::OfficialVendorUrl,
            "fabric-installer",
            mc_version,
            false,
            vec![format!(
                "Installed Fabric loader for Minecraft {mc_version} into {}",
                mc_dir.display()
            )],
            (!created.is_empty()).then(|| {
                format!(
                    "delete {}",
                    created.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
                )
            }),
        );
        receipt.created_paths = created.clone();
        self.record(
            plan,
            step,
            options,
            format!("Ran Fabric installer for Minecraft {mc_version}"),
            &receipt,
            (!created.is_empty()).then_some(RollbackData::CreatedPaths { paths: created }),
        )?;
        self.events.emit(AgentEvent::StepExecutionCompleted {
            step_id: step.id.clone(),
            receipt: receipt.clone(),
        });
        let verification = verify_fabric_profile(&mc_dir, mc_version);
        Ok(StepExecutionResult::done(step, receipt, Some(verification)))
    }

    async fn add_discovered_tool(
        &self,
        plan: &Plan,
        step: &PlannedStep,
        options: &ExecutionOptions,
        binary: AllowedBinary,
    ) -> Result<StepExecutionResult, CoreError> {
        let name = binary.executable_name();
        let version_args = [binary.version_flag().to_string()];
        let paths = self.os_provider.refresh_and_get_path().await?;

        if self.os_provider.which_in_paths(name, &paths).is_some() {
            let receipt = ExecutionReceipt::new(
                AdapterKind::Winget,
                "PATH",
                "registry",
                options.dry_run,
                vec![format!("{name} is already on PATH; nothing to change")],
                None,
            );
            self.events.emit(AgentEvent::StepExecutionCompleted {
                step_id: step.id.clone(),
                receipt: receipt.clone(),
            });
            let verification = self.verifier.verify_binary_command(&step.id, binary, &version_args).await?;
            return Ok(StepExecutionResult::done(step, receipt, Some(verification)));
        }

        let dir = match crate::discovery::discover_tool_dir(self.os_provider.as_ref(), binary).await {
            Some(dir) => dir,
            None if options.dry_run => {
                let receipt = ExecutionReceipt::new(
                    AdapterKind::Winget,
                    "PATH",
                    "registry",
                    true,
                    vec![format!("[Dry Run] Would locate {name} after install and add its folder to the User PATH")],
                    None,
                );
                self.events.emit(AgentEvent::StepExecutionCompleted {
                    step_id: step.id.clone(),
                    receipt: receipt.clone(),
                });
                return Ok(StepExecutionResult::done(step, receipt, None));
            }
            None => {
                return Err(CoreError::Os(format!(
                    "Could not find {name} in its registry install key, default install folders, or winget packages"
                )))
            }
        };

        let receipt = self.add_path_entry(plan, step, options, &dir).await?;
        self.events.emit(AgentEvent::StepExecutionCompleted {
            step_id: step.id.clone(),
            receipt: receipt.clone(),
        });
        let verification = if options.dry_run {
            None
        } else {
            Some(self.verifier.verify_binary_command(&step.id, binary, &version_args).await?)
        };
        Ok(StepExecutionResult::done(step, receipt, verification))
    }
}

fn list_dir_names(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Final hash gate for a downloaded file. With a SHA-256 the file must match it (and is
/// deleted when it does not); otherwise the adapter must already have enforced a published
/// SHA-512. Returns a passed verification only when a digest matched.
fn enforce_download_hash(
    path: &Path,
    expected_sha256: Option<&str>,
    computed_sha256: Option<&str>,
) -> Result<VerificationResult, CoreError> {
    match expected_sha256 {
        Some(expected) => match verify_file_sha256(path, expected) {
            Ok(actual) => Ok(VerificationResult {
                passed: true,
                command_run: format!("sha256 {}", path.display()),
                exit_code: Some(0),
                stdout: format!("SHA-256 matched {actual}"),
                stderr: String::new(),
            }),
            Err(e) => {
                let _ = std::fs::remove_file(path);
                Err(CoreError::Adapter(e))
            }
        },
        None => match computed_sha256 {
            Some(actual) if path.is_file() => Ok(VerificationResult {
                passed: true,
                command_run: format!("sha512 {}", path.display()),
                exit_code: Some(0),
                stdout: format!("SHA-512 matched the published digest (SHA-256 {actual})"),
                stderr: String::new(),
            }),
            _ => {
                let _ = std::fs::remove_file(path);
                Err(CoreError::Adapter(AdapterError::Other(format!(
                    "{} was not verified against any published checksum",
                    path.display()
                ))))
            }
        },
    }
}

#[cfg(test)]
mod tests;
