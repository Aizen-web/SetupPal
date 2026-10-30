use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::config::CoreError;
use crate::events::{AgentEvent, EventBus};
use crate::executor::{ExecutionOptions, Executor, PlanExecutionSummary};
use crate::llm::LlmClient;
use crate::safety::{PlanSafetyReport, RiskRating, SafetyEngine};
use crate::types::{Plan, PlannedStep, SystemProfile};

pub const DEFAULT_MAX_RECOVERY_ATTEMPTS: u8 = 2;

/// Rough failure class so remediations must change the input that caused it.
pub fn classify_failure(failure: &FailureContext) -> &'static str {
    let blob = format!("{} {}", failure.stderr, failure.stdout).to_ascii_lowercase();
    let code = failure.exit_code.map(|c| c as u32);
    if code == Some(0x8A150010) || blob.contains("no applicable installer") {
        return "scope";
    }
    if blob.contains("not found") || blob.contains("no package") || code == Some(0x8A150011) {
        return "not_found";
    }
    if blob.contains("timed out") || blob.contains("timeout") {
        return "timeout";
    }
    if blob.contains("already installed") {
        return "already_installed";
    }
    if blob.contains("hash") {
        return "hash";
    }
    if blob.contains("admin") {
        return "admin";
    }
    "other"
}

pub(crate) fn remediation_is_noop(plan: &Plan, failure: &FailureContext) -> bool {
    if plan.steps.is_empty() {
        return true;
    }
    // Only CreateIsolatedEnv with no install is never a fix for a winget/npm failure.
    let only_env = plan.steps.iter().all(|s| {
        matches!(
            &s.step,
            crate::types::Step::CreateIsolatedEnv { .. }
        )
    });
    if only_env {
        return true;
    }
    // Identical single-step install of the same package id mentioned in the failure.
    if plan.steps.len() == 1 {
        if let crate::types::Step::InstallPackage { package_id, .. } = &plan.steps[0].step {
            let blob = format!("{} {}", failure.stderr, failure.stdout).to_ascii_lowercase();
            if blob.contains(&package_id.to_ascii_lowercase())
                && (blob.contains("not found") || classify_failure(failure) == "not_found")
            {
                return true;
            }
        }
    }
    false
}

/// Diagnostic failure context captured during execution or verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureContext {
    pub failed_step_id: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub system_profile: SystemProfile,
    pub successful_previous_steps: Vec<PlannedStep>,
    pub attempt: u8,
}

impl FailureContext {
    pub fn new(
        failed_step_id: impl Into<String>,
        exit_code: Option<i32>,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
        system_profile: SystemProfile,
        successful_previous_steps: Vec<PlannedStep>,
        attempt: u8,
    ) -> Self {
        Self {
            failed_step_id: failed_step_id.into(),
            exit_code,
            stdout: stdout.into(),
            stderr: stderr.into(),
            system_profile,
            successful_previous_steps,
            attempt,
        }
    }

    pub fn formatted_diagnostic(&self) -> String {
        let successful_ids: Vec<String> = self
            .successful_previous_steps
            .iter()
            .map(|s| s.id.clone())
            .collect();

        format!(
            "Failed Step: {}\nExit Code: {:?}\nStderr: {}\nStdout: {}\nSuccessful Preceding Steps: [{}]",
            self.failed_step_id,
            self.exit_code,
            self.stderr.trim(),
            self.stdout.trim(),
            successful_ids.join(", ")
        )
    }
}

/// Zero-Trust Self-Healing Loop.
/// Diagnoses execution and verification failures, generates targeted remediation plans via NVIDIA NIM,
/// enforces mandatory safety gates, and limits attempts to prevent infinite retry loops.
pub struct Healer {
    llm: Arc<dyn LlmClient>,
    safety_engine: Arc<SafetyEngine>,
    events: EventBus,
    max_attempts: u8,
}

impl Healer {
    pub fn new(
        llm: Arc<dyn LlmClient>,
        safety_engine: Arc<SafetyEngine>,
        events: EventBus,
    ) -> Self {
        Self {
            llm,
            safety_engine,
            events,
            max_attempts: DEFAULT_MAX_RECOVERY_ATTEMPTS,
        }
    }

    pub fn with_max_attempts(mut self, max: u8) -> Self {
        self.max_attempts = max;
        self
    }

    /// Captures diagnostic context, requests a remediation plan, and passes it through `SafetyEngine`.
    pub async fn diagnose_and_propose(
        &self,
        goal: &str,
        failure: &FailureContext,
    ) -> Result<Option<(Plan, PlanSafetyReport)>, CoreError> {
        // Enforce loop limit
        if failure.attempt > self.max_attempts {
            tracing::warn!(
                "Self-healing loop limit exceeded (attempt {} > max {}) for step {}",
                failure.attempt,
                self.max_attempts,
                failure.failed_step_id
            );
            return Ok(None);
        }

        let diagnostic = failure.formatted_diagnostic();

        // 1. Emit SelfHealTriggered event
        self.events.emit(AgentEvent::SelfHealTriggered {
            failed_step_id: failure.failed_step_id.clone(),
            diagnostic_summary: diagnostic.clone(),
            attempt: failure.attempt,
        });

        // 2. Request remediation plan from NIM / LLM (include a classified cause)
        let cause = classify_failure(failure);
        let diagnostic_with_cause = format!("Classified cause: {cause}\n{diagnostic}");
        let remediation_plan = self
            .llm
            .propose_remediation_plan(
                goal,
                &failure.system_profile,
                &failure.failed_step_id,
                &diagnostic_with_cause,
                failure.attempt,
            )
            .await?;

        // Drop remediations that only repeat the failed step or add nothing useful (R12).
        if remediation_is_noop(&remediation_plan, failure) {
            tracing::warn!(
                "Healer proposed a no-op remediation for {}; showing the honest failure instead",
                failure.failed_step_id
            );
            return Ok(None);
        }

        // 3. Mandatory Zero-Trust Safety Gate
        let safety_report = self.safety_engine.evaluate_plan(&remediation_plan).await?;

        // 4. Emit SelfHealPlanProposed event
        self.events.emit(AgentEvent::SelfHealPlanProposed {
            remediation_plan: Box::new(remediation_plan.clone()),
            safety_report: Box::new(safety_report.clone()),
        });

        Ok(Some((remediation_plan, safety_report)))
    }

    /// Executes recovery if the proposed remediation plan passes the safety gate (not RED).
    pub async fn recover_and_execute(
        &self,
        executor: &Executor,
        goal: &str,
        failure: &FailureContext,
        options: &ExecutionOptions,
    ) -> Result<Option<PlanExecutionSummary>, CoreError> {
        let proposed = self.diagnose_and_propose(goal, failure).await?;
        let (plan, safety_report) = match proposed {
            Some(pair) => pair,
            None => return Ok(None),
        };

        // If the remediation plan is rated Red, zero-trust gate blocks execution
        if safety_report.overall_rating == RiskRating::Red {
            tracing::error!(
                "Remediation plan for {} scored RED risk and was blocked.",
                failure.failed_step_id
            );
            return Ok(None);
        }

        let summary = executor
            .execute_plan(&plan, &safety_report, options)
            .await?;

        Ok(Some(summary))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use async_trait::async_trait;

    use super::*;
    use crate::adapters::AdapterRegistry;
    use crate::llm::mock::{llm_plan, sample_profile, verify_step, MockLlmClient, RecordedLlmCall};
    use crate::os::mock::MockOsProvider;
    use crate::safety::osv::OsvClient;
    use crate::safety::VulnerabilitySummary;
    use crate::state::StateLogger;
    use crate::types::{AllowedBinary, Ecosystem, PlanSource, Step};
    use crate::verifier::Verifier;

    struct CleanOsv;

    #[async_trait]
    impl OsvClient for CleanOsv {
        async fn query_package(
            &self,
            _ecosystem: Ecosystem,
            _package_name: &str,
            _version: Option<&str>,
        ) -> Result<Vec<VulnerabilitySummary>, CoreError> {
            Ok(Vec::new())
        }
    }

    fn engine(events: &EventBus) -> Arc<SafetyEngine> {
        Arc::new(SafetyEngine::new(
            AdapterRegistry::new(),
            Arc::new(CleanOsv),
            None,
            events.clone(),
        ))
    }

    fn failure(attempt: u8) -> FailureContext {
        FailureContext::new(
            "step-2",
            Some(1),
            "partial output",
            "Access is denied",
            sample_profile(),
            vec![verify_step("step-1", AllowedBinary::Git)],
            attempt,
        )
    }

    fn executor(os: Arc<MockOsProvider>, events: &EventBus) -> Executor {
        let log =
            std::env::temp_dir().join(format!("setuppal-heal-{}.jsonl", uuid::Uuid::new_v4()));
        Executor::new(
            AdapterRegistry::with_system_defaults(os.clone()),
            os.clone(),
            StateLogger::new(log, events.clone()),
            Verifier::new(os, events.clone()),
            events.clone(),
        )
    }

    fn green_fix() -> Plan {
        llm_plan("fix", vec![verify_step("fix-1", AllowedBinary::Git)])
    }

    fn red_fix() -> Plan {
        llm_plan(
            "fix",
            vec![crate::types::PlannedStep {
                id: "fix-1".to_string(),
                description: "Verify a download".to_string(),
                step: Step::VerifyHash {
                    file_path: PathBuf::from("installer.exe"),
                    expected_sha256: "0".repeat(64),
                },
            }],
        )
    }

    #[test]
    fn diagnostic_text_carries_the_real_failure() {
        let text = failure(1).formatted_diagnostic();
        assert!(text.contains("Failed Step: step-2"));
        assert!(text.contains("Access is denied"));
        assert!(text.contains("partial output"));
        assert!(text.contains("step-1"));
    }

    #[test]
    fn noop_remediation_is_dropped_for_not_found_and_isolated_env_only() {
        let not_found = FailureContext::new(
            "step-1",
            Some(0x8A150011u32 as i32),
            "",
            "No package found matching input criteria for ripgrep",
            sample_profile(),
            vec![],
            1,
        );
        let same_pkg = llm_plan(
            "fix",
            vec![crate::types::PlannedStep {
                id: "fix-1".to_string(),
                description: "Install ripgrep".to_string(),
                step: Step::InstallPackage {
                    ecosystem: Ecosystem::Winget,
                    package_id: "ripgrep".to_string(),
                    version: None,
                    source_adapter: crate::types::AdapterKind::Winget,
                },
            }],
        );
        assert!(remediation_is_noop(&same_pkg, &not_found));

        let only_env = llm_plan(
            "fix",
            vec![crate::types::PlannedStep {
                id: "fix-1".to_string(),
                description: "Create env".to_string(),
                step: Step::CreateIsolatedEnv {
                    kind: crate::types::IsolationKind::PythonVenv,
                    path: std::path::PathBuf::from(r"C:\Users\me\.venvs\x"),
                },
            }],
        );
        assert!(remediation_is_noop(&only_env, &not_found));

        let different = llm_plan(
            "fix",
            vec![crate::types::PlannedStep {
                id: "fix-1".to_string(),
                description: "Install BurntSushi.ripgrep.MSVC".to_string(),
                step: Step::InstallPackage {
                    ecosystem: Ecosystem::Winget,
                    package_id: "BurntSushi.ripgrep.MSVC".to_string(),
                    version: None,
                    source_adapter: crate::types::AdapterKind::Winget,
                },
            }],
        );
        assert!(!remediation_is_noop(&different, &not_found));
    }

    #[tokio::test]
    async fn remediation_goes_through_the_model_then_the_safety_gate() {
        let events = EventBus::default();
        let mut rx = events.subscribe();
        let llm = Arc::new(MockLlmClient::new().with_remediation(green_fix()));
        let healer = Healer::new(llm.clone(), engine(&events), events.clone());

        let (plan, report) = healer
            .diagnose_and_propose("install git", &failure(1))
            .await
            .expect("no error")
            .expect("a proposal");

        assert_eq!(
            plan.source,
            PlanSource::Healer {
                failed_step_id: "step-2".to_string(),
                attempt: 1
            }
        );
        assert_eq!(report.overall_rating, RiskRating::Green);
        match &llm.calls()[0] {
            RecordedLlmCall::Remediation {
                failed_step_id,
                diagnostic,
                attempt,
                ..
            } => {
                assert_eq!(failed_step_id, "step-2");
                assert!(diagnostic.contains("Access is denied"));
                assert_eq!(*attempt, 1);
            }
            other => panic!("unexpected call {other:?}"),
        }
        let (mut triggered, mut proposed) = (false, false);
        while let Ok(event) = rx.try_recv() {
            triggered |= matches!(event, AgentEvent::SelfHealTriggered { .. });
            proposed |= matches!(event, AgentEvent::SelfHealPlanProposed { .. });
        }
        assert!(triggered && proposed);
    }

    #[tokio::test]
    async fn attempts_past_the_limit_do_not_call_the_model() {
        let events = EventBus::default();
        let llm = Arc::new(MockLlmClient::new().with_remediation(green_fix()));
        let healer = Healer::new(llm.clone(), engine(&events), events).with_max_attempts(1);
        let proposal = healer
            .diagnose_and_propose("g", &failure(2))
            .await
            .expect("no error");
        assert!(proposal.is_none());
        assert!(llm.calls().is_empty());
    }

    #[tokio::test]
    async fn model_failure_is_reported_not_hidden() {
        let events = EventBus::default();
        let llm = Arc::new(MockLlmClient::new().with_remediation_error("rate limited"));
        let healer = Healer::new(llm, engine(&events), events);
        let err = healer
            .diagnose_and_propose("g", &failure(1))
            .await
            .expect_err("llm error");
        assert!(err.to_string().contains("rate limited"));
    }

    #[tokio::test]
    async fn red_remediation_is_blocked_and_never_executed() {
        let events = EventBus::default();
        let os = Arc::new(MockOsProvider::new());
        let llm = Arc::new(MockLlmClient::new().with_remediation(red_fix()));
        let healer = Healer::new(llm, engine(&events), events.clone());
        let options = ExecutionOptions {
            dry_run: true,
            auto_approve_green: true,
            ..ExecutionOptions::default()
        };
        let outcome = healer
            .recover_and_execute(&executor(os.clone(), &events), "g", &failure(1), &options)
            .await
            .expect("no error");
        assert!(outcome.is_none());
        assert!(os.calls().is_empty());
    }

    #[tokio::test]
    async fn green_remediation_executes_through_the_executor() {
        let events = EventBus::default();
        let os = Arc::new(MockOsProvider::new());
        let llm = Arc::new(MockLlmClient::new().with_remediation(green_fix()));
        let healer = Healer::new(llm, engine(&events), events.clone());
        let options = ExecutionOptions {
            dry_run: true,
            auto_approve_green: true,
            ..ExecutionOptions::default()
        };
        let summary = healer
            .recover_and_execute(&executor(os, &events), "g", &failure(1), &options)
            .await
            .expect("no error")
            .expect("ran");
        assert_eq!(summary.step_results.len(), 1);
        assert!(summary.step_results[0].executed);
    }
}
