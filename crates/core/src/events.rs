use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::adapters::{ExecutionReceipt, VerificationResult};
use crate::safety::{PlanSafetyReport, RiskRating, SafetyReport};
use crate::types::{Plan, SystemProfile};

/// Broadcast across `tokio::sync::broadcast::Sender<AgentEvent>` to drive
/// both the CLI terminal UI and Tauri v2 frontend in real time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "payload", rename_all = "snake_case")]
pub enum AgentEvent {
    ScanStarted,
    ScanCompleted {
        profile: Box<SystemProfile>,
    },
    PlanningStarted {
        goal: String,
    },
    RecipeMatched {
        recipe_id: String,
    },
    LlmPlanningFallback {
        goal: String,
        model: String,
    },
    PlanGenerated {
        plan: Box<Plan>,
    },
    SafetyCheckStarted {
        plan_id: String,
    },
    StepSafetyEvaluated {
        report: Box<SafetyReport>,
    },
    SafetyCheckCompleted {
        report: Box<PlanSafetyReport>,
    },
    StepApprovalRequired {
        step_id: String,
        rating: RiskRating,
        reasons: Vec<String>,
    },
    StepExecutionStarted {
        step_id: String,
        description: String,
        dry_run: bool,
    },
    StepExecutionProgress {
        step_id: String,
        message: String,
        bytes_downloaded: Option<u64>,
        total_bytes: Option<u64>,
    },
    StepExecutionCompleted {
        step_id: String,
        receipt: ExecutionReceipt,
    },
    StepExecutionBlocked {
        step_id: String,
        reason: String,
    },
    StepExecutionFailed {
        step_id: String,
        error: String,
    },
    ExecutionCancelled {
        run_id: String,
        step_id: String,
    },
    VerificationStarted {
        step_id: String,
    },
    VerificationCompleted {
        step_id: String,
        result: VerificationResult,
    },
    SelfHealTriggered {
        failed_step_id: String,
        diagnostic_summary: String,
        attempt: u8,
    },
    SelfHealPlanProposed {
        remediation_plan: Box<Plan>,
        safety_report: Box<PlanSafetyReport>,
    },
    TransactionRecorded {
        tx_id: String,
        action_summary: String,
    },
    RollbackStarted {
        tx_id: String,
    },
    RollbackCompleted {
        tx_id: String,
        success: bool,
        message: String,
    },
}

/// Thin wrapper around `tokio::sync::broadcast::Sender<AgentEvent>`
/// so modules can emit events without failing if no receiver is currently listening.
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }

    pub fn emit(&self, event: AgentEvent) {
        let _ = self.sender.send(event);
    }
}
