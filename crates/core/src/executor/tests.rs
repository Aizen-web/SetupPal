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
use crate::os::OsProvider;
use crate::safety::hash::compute_file_sha256;
use crate::safety::{PlanSafetyReport, RiskRating, SafetyReport, SafetySignals};
use crate::state::StateLogger;
use crate::types::{Plan, PlanSource, PlannedStep, Step};
use crate::verifier::Verifier;

fn write_temp(bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("setuppal-hash-test-{}", uuid::Uuid::new_v4()));
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
        notes: Vec::new(),
    }
}

fn test_executor() -> Executor {
    let os = Arc::new(MockOsProvider::new());
    let events = EventBus::default();
    let log_path =
        std::env::temp_dir().join(format!("setuppal-exec-test-{}.jsonl", uuid::Uuid::new_v4()));
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
async fn yellow_not_in_allow_list_is_skipped() {
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
    assert!(summary.halted_at_step_id.is_none());
    assert_eq!(summary.step_results[0].status, StepRunStatus::Skipped);
    assert!(!summary.step_results[0].executed);
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
async fn green_without_approval_is_skipped() {
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
    assert!(summary.halted_at_step_id.is_none());
    assert_eq!(summary.step_results[0].status, StepRunStatus::Skipped);
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

fn executor_with(os: Arc<MockOsProvider>) -> (Executor, StateLogger) {
    let events = EventBus::default();
    let log_path =
        std::env::temp_dir().join(format!("setuppal-exec-test-{}.jsonl", uuid::Uuid::new_v4()));
    let state_logger = StateLogger::new(log_path, events.clone());
    let verifier = Verifier::new(os.clone(), events.clone());
    let executor = Executor::new(
        AdapterRegistry::with_system_defaults(os.clone()),
        os,
        state_logger.clone(),
        verifier,
        events,
    );
    (executor, state_logger)
}

fn verify_binary_step(id: &str) -> PlannedStep {
    PlannedStep {
        id: id.to_string(),
        description: format!("verify {id}"),
        step: Step::RunVerifyCommand {
            binary: crate::types::AllowedBinary::Git,
            args: vec!["--version".to_string()],
        },
    }
}

#[tokio::test]
async fn live_green_step_runs_verifies_and_is_recorded_for_undo() {
    let os = Arc::new(MockOsProvider::new());
    let (executor, logger) = executor_with(os.clone());
    let plan = sample_plan(vec![sample_step("step-1")]);
    let safety = plan_report(&[("step-1", RiskRating::Green)]);
    let options = ExecutionOptions {
        dry_run: false,
        auto_approve_green: true,
        ..ExecutionOptions::default()
    };
    let summary = executor
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert!(summary.overall_passed, "{summary:?}");
    assert!(summary.step_results[0].executed);

    let records = logger.read_all_transactions().expect("log");
    assert_eq!(records.len(), 1);
    assert!(!records[0].dry_run);
    assert!(
        records[0].rollback_data.is_some(),
        "a PATH change must be undoable"
    );

    let dyn_os: Arc<dyn crate::os::OsProvider> = os.clone();
    logger
        .rollback_transaction(&records[0], &dyn_os)
        .await
        .expect("undo");
    let path = os.refresh_and_get_path().await.expect("path");
    assert!(!path.contains(&PathBuf::from("C:\\Tools\\Sample")));
}

#[tokio::test]
async fn approved_green_step_runs_without_auto_approve() {
    let (executor, _) = executor_with(Arc::new(MockOsProvider::new()));
    let plan = sample_plan(vec![sample_step("step-1")]);
    let safety = plan_report(&[("step-1", RiskRating::Green)]);
    let options = ExecutionOptions {
        dry_run: true,
        auto_approve_green: false,
        approved_step_ids: HashSet::from(["step-1".to_string()]),
        ..ExecutionOptions::default()
    };
    let summary = executor
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert!(summary.halted_at_step_id.is_none());
    assert!(summary.step_results[0].executed);
}

#[tokio::test]
async fn unticked_yellow_is_skipped_and_later_approved_steps_still_run() {
    let (executor, _) = executor_with(Arc::new(MockOsProvider::new()));
    let plan = sample_plan(vec![
        sample_step("step-1"),
        sample_step("step-2"),
        sample_step("step-3"),
    ]);
    let safety = plan_report(&[
        ("step-1", RiskRating::Yellow),
        ("step-2", RiskRating::Green),
        ("step-3", RiskRating::Yellow),
    ]);
    let options = ExecutionOptions {
        dry_run: true,
        auto_approve_green: true,
        allow_yellow_steps: HashSet::new(), // none ticked
        ..ExecutionOptions::default()
    };
    let summary = executor
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert!(summary.halted_at_step_id.is_none());
    assert_eq!(summary.step_results[0].status, StepRunStatus::Skipped);
    assert!(summary.step_results[1].executed);
    assert_eq!(summary.step_results[2].status, StepRunStatus::Skipped);
    assert!(summary.overall_passed);
}

#[tokio::test]
async fn approving_the_held_step_resumes_without_rerunning_earlier_steps() {
    let (executor, logger) = executor_with(Arc::new(MockOsProvider::new()));
    let plan = sample_plan(vec![sample_step("step-1"), sample_step("step-2")]);
    let safety = plan_report(&[
        ("step-1", RiskRating::Green),
        ("step-2", RiskRating::Yellow),
    ]);
    let options = ExecutionOptions {
        dry_run: false,
        auto_approve_green: true,
        allow_yellow_steps: HashSet::from(["step-2".to_string()]),
        start_at_step_id: Some("step-2".to_string()),
        ..ExecutionOptions::default()
    };
    let summary = executor
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert!(summary.halted_at_step_id.is_none());
    let records = logger.read_all_transactions().expect("log");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].step_id, "step-2");
}

#[tokio::test]
async fn a_blocked_red_step_is_not_recorded_as_a_change() {
    let (executor, logger) = executor_with(Arc::new(MockOsProvider::new()));
    let plan = sample_plan(vec![sample_step("step-1")]);
    let safety = plan_report(&[("step-1", RiskRating::Red)]);
    let options = ExecutionOptions {
        dry_run: false,
        auto_approve_green: true,
        ..ExecutionOptions::default()
    };
    let summary = executor
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert_eq!(summary.step_results[0].status, StepRunStatus::Blocked);
    assert!(logger.read_all_transactions().expect("log").is_empty());
}

#[tokio::test]
async fn failing_verification_makes_the_run_fail_honestly() {
    let os = Arc::new(
        MockOsProvider::new()
            .with_binary("git", "C:/mock/git.exe")
            .on(
                "git",
                &["--version"],
                crate::os::mock::fail(1, "git is broken"),
            ),
    );
    let (executor, _) = executor_with(os);
    let plan = sample_plan(vec![verify_binary_step("step-1")]);
    let safety = plan_report(&[("step-1", RiskRating::Green)]);
    let options = ExecutionOptions {
        dry_run: false,
        auto_approve_green: true,
        ..ExecutionOptions::default()
    };
    let summary = executor
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert!(!summary.overall_passed);
    assert!(!summary.step_results[0].succeeded());
}

#[tokio::test]
async fn passing_verification_makes_the_run_pass() {
    let os = Arc::new(
        MockOsProvider::new()
            .with_binary("git", "C:/mock/git.exe")
            .on(
                "git",
                &["--version"],
                crate::os::mock::ok("git version 2.50.0"),
            ),
    );
    let (executor, _) = executor_with(os);
    let plan = sample_plan(vec![verify_binary_step("step-1")]);
    let safety = plan_report(&[("step-1", RiskRating::Green)]);
    let options = ExecutionOptions {
        dry_run: false,
        auto_approve_green: true,
        ..ExecutionOptions::default()
    };
    let summary = executor
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert!(summary.overall_passed, "{summary:?}");
}

#[test]
fn trusted_categories_cover_only_their_own_step_kinds() {
    use crate::executor::TrustCategory;
    use crate::types::{AdapterKind, Ecosystem};
    let options = ExecutionOptions {
        trusted_categories: HashSet::from([TrustCategory::VerifiedWinget]),
        ..ExecutionOptions::default()
    };
    let winget = Step::InstallPackage {
        ecosystem: Ecosystem::Winget,
        package_id: "Git.Git".to_string(),
        version: None,
        source_adapter: AdapterKind::Winget,
    };
    let npm = Step::InstallPackage {
        ecosystem: Ecosystem::Npm,
        package_id: "typescript".to_string(),
        version: None,
        source_adapter: AdapterKind::Npm,
    };
    assert!(options.is_step_trusted(&winget));
    assert!(!options.is_step_trusted(&npm));
    assert!(!options.is_step_trusted(&sample_step("x").step));
}

#[tokio::test]
async fn dry_run_summary_headline_says_dry_run_and_nothing_changed() {
    let plan = sample_plan(vec![sample_step("step-1")]);
    let safety = plan_report(&[("step-1", RiskRating::Green)]);
    let options = ExecutionOptions {
        dry_run: true,
        auto_approve_green: true,
        ..ExecutionOptions::default()
    };
    let executor = test_executor();
    let mut events = executor.events.subscribe();
    let summary = executor
        .execute_plan(&plan, &safety, &options)
        .await
        .expect("summary");
    assert!(summary.dry_run);
    let headline = summary.headline();
    assert!(headline.starts_with("DRY RUN"), "{headline}");
    assert!(headline.contains("Nothing was installed"), "{headline}");

    let mut saw_started = false;
    while let Ok(event) = events.try_recv() {
        if let crate::events::AgentEvent::StepExecutionStarted { dry_run, .. } = event {
            assert!(dry_run, "dry run steps must announce dry_run: true");
            saw_started = true;
        }
    }
    assert!(saw_started);
}

#[test]
fn live_summary_headline_never_mentions_dry_run() {
    let mut summary = super::PlanExecutionSummary {
        plan_id: "p".into(),
        goal: "g".into(),
        dry_run: false,
        step_results: vec![],
        overall_passed: true,
        toolchain_verification: None,
        halted_at_step_id: None,
        cancelled: false,
    };
    assert!(!summary.headline().to_ascii_lowercase().contains("dry run"));
    summary.overall_passed = false;
    assert!(summary.headline().starts_with("Run failed"));
    summary.dry_run = true;
    summary.halted_at_step_id = Some("s".into());
    assert!(summary.headline().starts_with("DRY RUN paused"));
}
