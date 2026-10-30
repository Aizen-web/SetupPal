use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use super::{enforce_download_hash, ExecutionOptions, Executor, StepRunStatus};
use crate::adapters::{AdapterError, AdapterRegistry};
use crate::config::CoreError;
use crate::events::EventBus;
use crate::os::mock::MockOsProvider;
use crate::safety::hash::compute_file_sha256;
use crate::safety::{PlanSafetyReport, RiskRating, SafetyReport, SafetySignals};
use crate::state::StateLogger;
use crate::types::{Plan, PlanSource, PlannedStep, Step};
use crate::verifier::Verifier;

fn write_temp(bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "setuppal-hash-test-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("artifact.bin");
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(bytes).expect("write");
    path
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[test]
fn enforce_download_hash_accepts_matching_sha256() {
    let bytes = b"setuppal-real-payload";
    let path = write_temp(bytes);
    let expected = sha256_hex(bytes);

    let result = enforce_download_hash(&path, Some(&expected), None).expect("hash ok");
    assert!(result.passed);
    assert!(path.is_file(), "matching file must remain");
    assert_eq!(compute_file_sha256(&path).unwrap(), expected);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(path.parent().unwrap());
}

#[test]
fn enforce_download_hash_rejects_mismatch_and_deletes_file() {
    let path = write_temp(b"setuppal-real-payload");
    let wrong = sha256_hex(b"different-bytes");

    let err = enforce_download_hash(&path, Some(&wrong), None).expect_err("mismatch");
    match err {
        CoreError::Adapter(AdapterError::HashMismatch { .. }) => {}
        other => panic!("expected HashMismatch, got {other:?}"),
    }
    assert!(!path.exists(), "mismatched download must be deleted");

    let _ = std::fs::remove_dir(path.parent().unwrap());
}

#[test]
fn enforce_download_hash_requires_some_digest() {
    let path = write_temp(b"no-digest");
    let err = enforce_download_hash(&path, None, None).expect_err("no digest");
    assert!(matches!(err, CoreError::Adapter(_)));
    assert!(!path.exists(), "unverified download must be deleted");
    let _ = std::fs::remove_dir(path.parent().unwrap());
}

fn sample_step(id: &str) -> PlannedStep {
    PlannedStep {
        id: id.to_string(),
        description: format!("step {id}"),
        step: Step::AddToUserPath {
            directory: PathBuf::from("C:\\Tools\\Sample"),
        },
    }
}

fn sample_plan(steps: Vec<PlannedStep>) -> Plan {
    Plan {
        plan_id: "plan-test".to_string(),
        goal: "test".to_string(),
        summary: "test".to_string(),
        source: PlanSource::Llm {
            model: "test".to_string(),
        },
        steps,
    }
}

fn report_for(step_id: &str, rating: RiskRating) -> SafetyReport {
    SafetyReport {
        step_id: step_id.to_string(),
        rating,
        risk_score: match rating {
            RiskRating::Green => 10,
            RiskRating::Yellow => 40,
            RiskRating::Red => 90,
        },
        reasons: vec![format!("{rating:?} reason")],
        suggested_alternative: None,
        signals: SafetySignals::default(),
    }
}

fn plan_report(steps: &[(&str, RiskRating)]) -> PlanSafetyReport {
    let step_reports: Vec<_> = steps
        .iter()
        .map(|(id, rating)| report_for(id, *rating))
        .collect();
    let overall = step_reports
        .iter()
        .map(|r| r.rating)
        .max()
        .unwrap_or(RiskRating::Green);
    PlanSafetyReport {
        plan_id: "plan-test".to_string(),
        overall_rating: overall,
        overall_score: 50,
        step_reports,
        can_execute: overall != RiskRating::Red,
    }
}

fn test_executor() -> Executor {
    let os = Arc::new(MockOsProvider::new());
    let events = EventBus::default();
    let log_path = std::env::temp_dir().join(format!(
        "setuppal-exec-test-{}.jsonl",
        uuid::Uuid::new_v4()
    ));
    let state_logger = StateLogger::new(log_path, events.clone());
    let verifier = Verifier::new(os.clone(), events.clone());
    Executor::new(
        AdapterRegistry::with_system_defaults(os.clone()),
        os,
        state_logger,
        verifier,
        events,
    )
}

#[tokio::test]
async fn yellow_not_in_allow_list_halts() {
    let plan = sample_plan(vec![sample_step("step-1")]);
    let safety = plan_report(&[("step-1", RiskRating::Yellow)]);
    let options = ExecutionOptions {
        dry_run: true,
        ..ExecutionOptions::default()
    };
    let summary = test_executor()
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert_eq!(summary.halted_at_step_id.as_deref(), Some("step-1"));
    assert_eq!(summary.step_results[0].status, StepRunStatus::Held);
    assert!(!summary.overall_passed);
}

#[tokio::test]
async fn yellow_in_allow_list_runs() {
    let plan = sample_plan(vec![sample_step("step-1")]);
    let safety = plan_report(&[("step-1", RiskRating::Yellow)]);
    let options = ExecutionOptions {
        dry_run: true,
        allow_yellow_steps: HashSet::from(["step-1".to_string()]),
        ..ExecutionOptions::default()
    };
    let summary = test_executor()
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert!(summary.halted_at_step_id.is_none());
    assert!(summary.step_results[0].succeeded());
    assert!(summary.overall_passed);
}

#[tokio::test]
async fn green_without_approval_halts() {
    let plan = sample_plan(vec![sample_step("step-1")]);
    let safety = plan_report(&[("step-1", RiskRating::Green)]);
    let options = ExecutionOptions {
        dry_run: false,
        auto_approve_green: false,
        ..ExecutionOptions::default()
    };
    let summary = test_executor()
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert_eq!(summary.halted_at_step_id.as_deref(), Some("step-1"));
    assert_eq!(summary.step_results[0].status, StepRunStatus::Held);
}

#[tokio::test]
async fn red_never_runs_even_when_listed() {
    let plan = sample_plan(vec![sample_step("step-1")]);
    let safety = plan_report(&[("step-1", RiskRating::Red)]);
    let options = ExecutionOptions {
        dry_run: true,
        approved_step_ids: HashSet::from(["step-1".to_string()]),
        allow_yellow_steps: HashSet::from(["step-1".to_string()]),
        auto_approve_green: true,
        ..ExecutionOptions::default()
    };
    let summary = test_executor()
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert_eq!(summary.step_results[0].status, StepRunStatus::Blocked);
    assert!(!summary.overall_passed);
}

#[tokio::test]
async fn cancel_before_step_marks_cancelled() {
    let plan = sample_plan(vec![sample_step("step-1")]);
    let safety = plan_report(&[("step-1", RiskRating::Green)]);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let options = ExecutionOptions {
        dry_run: true,
        auto_approve_green: true,
        cancel,
        run_id: "run-1".to_string(),
        ..ExecutionOptions::default()
    };
    let summary = test_executor()
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert!(summary.cancelled);
    assert_eq!(summary.step_results[0].status, StepRunStatus::Cancelled);
}
