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
