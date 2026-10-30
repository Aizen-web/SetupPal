//! Scripted `LlmClient` for offline tests. It never touches the network or a key.

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::config::CoreError;
use crate::llm::LlmClient;
use crate::types::{AllowedBinary, Plan, PlanSource, PlannedStep, Step, SystemProfile};

/// One recorded call to the mock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedLlmCall {
    Plan {
        goal: String,
        untrusted_context: Option<String>,
    },
    Remediation {
        goal: String,
        failed_step_id: String,
        diagnostic: String,
        attempt: u8,
    },
}

/// Returns queued plans in order and records every request. An empty queue is an error,
/// so a test that triggers an unexpected extra model call fails loudly.
#[derive(Default)]
pub struct MockLlmClient {
    plans: Mutex<VecDeque<Result<Plan, String>>>,
    remediations: Mutex<VecDeque<Result<Plan, String>>>,
    calls: Mutex<Vec<RecordedLlmCall>>,
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_plan(self, plan: Plan) -> Self {
        if let Ok(mut q) = self.plans.lock() {
            q.push_back(Ok(plan));
        }
        self
    }

    pub fn with_plan_error(self, message: &str) -> Self {
        if let Ok(mut q) = self.plans.lock() {
            q.push_back(Err(message.to_string()));
        }
        self
    }

    pub fn with_remediation(self, plan: Plan) -> Self {
        if let Ok(mut q) = self.remediations.lock() {
            q.push_back(Ok(plan));
        }
        self
    }

    pub fn with_remediation_error(self, message: &str) -> Self {
        if let Ok(mut q) = self.remediations.lock() {
            q.push_back(Err(message.to_string()));
        }
        self
    }

    pub fn calls(&self) -> Vec<RecordedLlmCall> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }

    fn record(&self, call: RecordedLlmCall) {
        if let Ok(mut c) = self.calls.lock() {
            c.push(call);
        }
    }

    fn next(queue: &Mutex<VecDeque<Result<Plan, String>>>, what: &str) -> Result<Plan, CoreError> {
        let next = queue
            .lock()
            .map_err(|_| CoreError::Llm("mock queue lock poisoned".to_string()))?
            .pop_front();
        match next {
            Some(Ok(plan)) => Ok(plan),
            Some(Err(message)) => Err(CoreError::Llm(message)),
            None => Err(CoreError::Llm(format!(
                "mock: no scripted {what} plan left"
            ))),
        }
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    fn model_name(&self) -> &str {
        "mock-model"
    }

    async fn propose_plan(
        &self,
        goal: &str,
        _profile: &SystemProfile,
        untrusted_context: Option<&str>,
    ) -> Result<Plan, CoreError> {
        self.record(RecordedLlmCall::Plan {
            goal: goal.to_string(),
            untrusted_context: untrusted_context.map(str::to_string),
        });
        Self::next(&self.plans, "plan")
    }

    async fn propose_remediation_plan(
        &self,
        goal: &str,
        _profile: &SystemProfile,
        failed_step_id: &str,
        diagnostic: &str,
        attempt: u8,
    ) -> Result<Plan, CoreError> {
        self.record(RecordedLlmCall::Remediation {
            goal: goal.to_string(),
            failed_step_id: failed_step_id.to_string(),
            diagnostic: diagnostic.to_string(),
            attempt,
        });
        let mut plan = Self::next(&self.remediations, "remediation")?;
        plan.source = PlanSource::Healer {
            failed_step_id: failed_step_id.to_string(),
            attempt,
        };
        Ok(plan)
    }
}

/// A small, typical Windows machine for prompts and planner calls.
pub fn sample_profile() -> SystemProfile {
    SystemProfile {
        os_name: "windows".to_string(),
        os_version: "Mock Windows 11".to_string(),
        arch: "x86_64".to_string(),
        gpu_detected: None,
        free_disk_bytes: 50 * 1024 * 1024 * 1024,
        installed_tools: Vec::new(),
        active_path_entries: Vec::new(),
    }
}

/// A verify-only step (typed, never a raw command).
pub fn verify_step(id: &str, binary: AllowedBinary) -> PlannedStep {
    PlannedStep {
        id: id.to_string(),
        description: format!("Check {}", binary.executable_name()),
        step: Step::RunVerifyCommand {
            binary,
            args: vec![binary.version_flag().to_string()],
        },
    }
}

/// An LLM-sourced plan made of the given steps.
pub fn llm_plan(goal: &str, steps: Vec<PlannedStep>) -> Plan {
    Plan {
        plan_id: format!("plan-{}", uuid::Uuid::new_v4()),
        goal: goal.to_string(),
        summary: "mock plan".to_string(),
        source: PlanSource::Llm {
            model: "mock-model".to_string(),
        },
        steps,
    }
}
