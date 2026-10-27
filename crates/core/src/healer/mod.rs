use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::config::CoreError;
use crate::events::{AgentEvent, EventBus};
use crate::executor::{ExecutionOptions, Executor, PlanExecutionSummary};
use crate::llm::LlmClient;
use crate::safety::{PlanSafetyReport, RiskRating, SafetyEngine};
use crate::types::{Plan, PlannedStep, SystemProfile};

pub const DEFAULT_MAX_RECOVERY_ATTEMPTS: u8 = 2;

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

        // 2. Request remediation plan from NIM / LLM
        let remediation_plan = self
            .llm
            .propose_remediation_plan(
                goal,
                &failure.system_profile,
                &failure.failed_step_id,
                &diagnostic,
                failure.attempt,
            )
            .await?;

        // 3. Mandatory Zero-Trust Safety Gate
        let safety_report = self
            .safety_engine
            .evaluate_plan(&remediation_plan)
            .await?;

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
    use super::*;
    use std::path::PathBuf;
    use crate::adapters::AdapterRegistry;
    use crate::config::CoreConfig;
    use crate::llm::NimClient;
    use crate::os::MockOsProvider;
    use crate::state::StateLogger;
    use crate::verifier::Verifier;

    fn build_test_healer() -> (Healer, Executor, EventBus) {
        let events = EventBus::default();
        let os = Arc::new(MockOsProvider::new());
        let adapters = AdapterRegistry::with_default_mocks();
        let safety = Arc::new(SafetyEngine::new_offline(events.clone()));

        let mut config = CoreConfig::from_env();
        config.nim_mock = true;
        let llm = Arc::new(NimClient::new(config).unwrap());

        let healer = Healer::new(llm, safety, events.clone());

        let temp_state_log = tempfile::NamedTempFile::new().unwrap().path().to_path_buf();
        let state_logger = StateLogger::new(temp_state_log, events.clone());
        let verifier = Verifier::new(os.clone(), events.clone());
        let executor = Executor::new(adapters, os, state_logger, verifier, events.clone());

        (healer, executor, events)
    }

    #[tokio::test]
    async fn healer_proposes_remediation_plan_and_emits_events() {
        let (healer, _executor, events) = build_test_healer();
        let mut rx = events.subscribe();

        let profile = SystemProfile {
            os_name: "windows".to_string(),
            os_version: "11".to_string(),
            arch: "x86_64".to_string(),
            gpu_detected: None,
            free_disk_bytes: 100_000_000_000,
            installed_tools: vec![],
            active_path_entries: vec![PathBuf::from("C:\\Windows\\system32")],
        };

        let failure = FailureContext::new(
            "step-1",
            Some(1),
            "",
            "clang++: command not found",
            profile,
            vec![],
            1,
        );

        let result = healer.diagnose_and_propose("set up C++ toolchain", &failure).await.unwrap();
        assert!(result.is_some());

        let (plan, report) = result.unwrap();
        assert_eq!(plan.goal, "Remediate failure at step-1");
        assert!(matches!(plan.source, crate::types::PlanSource::Healer { .. }));
        assert_ne!(report.overall_rating, RiskRating::Red);

        // Verify events emitted
        let ev1 = rx.recv().await.unwrap();
        assert!(matches!(ev1, AgentEvent::SelfHealTriggered { .. }));
        let ev2 = rx.recv().await.unwrap();
        assert!(matches!(ev2, AgentEvent::SafetyCheckStarted { .. }));
    }

    #[tokio::test]
    async fn healer_respects_max_attempts_limit() {
        let (healer, _executor, _events) = build_test_healer();

        let profile = SystemProfile {
            os_name: "windows".to_string(),
            os_version: "11".to_string(),
            arch: "x86_64".to_string(),
            gpu_detected: None,
            free_disk_bytes: 100_000_000_000,
            installed_tools: vec![],
            active_path_entries: vec![],
        };

        let failure = FailureContext::new(
            "step-1",
            Some(1),
            "",
            "error",
            profile,
            vec![],
            3, // Attempt 3 exceeds default max of 2
        );

        let result = healer.diagnose_and_propose("goal", &failure).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn healer_safety_gate_blocks_malicious_remediation_red() {
        let (healer, executor, _events) = build_test_healer();

        let profile = SystemProfile {
            os_name: "windows".to_string(),
            os_version: "11".to_string(),
            arch: "x86_64".to_string(),
            gpu_detected: None,
            free_disk_bytes: 100_000_000_000,
            installed_tools: vec![],
            active_path_entries: vec![],
        };

        // Trigger diagnostic with poison keyword that causes mock LLM to propose reqwests typosquat
        let failure = FailureContext::new(
            "step-1",
            Some(1),
            "",
            "evil payload injection test",
            profile,
            vec![],
            1,
        );

        let proposed = healer.diagnose_and_propose("goal", &failure).await.unwrap();
        assert!(proposed.is_some());
        let (_plan, report) = proposed.unwrap();
        assert_eq!(report.overall_rating, RiskRating::Red);

        // Recover and execute should block it completely
        let exec_options = ExecutionOptions {
            dry_run: true,
            auto_approve_green: true,
            allow_yellow: true,
            working_dir: PathBuf::from("."),
            trusted_categories: std::collections::HashSet::new(),
        };

        let execution = healer
            .recover_and_execute(&executor, "goal", &failure, &exec_options)
            .await
            .unwrap();

        assert!(execution.is_none());
    }
}
