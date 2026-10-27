use std::path::PathBuf;

use crate::config::CoreError;
use crate::types::{
    AdapterKind, AllowedBinary, Ecosystem, IsolationKind, Plan, PlanSource, PlannedStep, Step,
    SystemProfile,
};

/// Deterministic offline NIM mock generator (`NIM_MOCK=1`).
/// Produces realistic, schema-compliant `Plan` structs for offline development and automated tests.
pub fn generate_mock_plan(
    goal: &str,
    _profile: &SystemProfile,
    untrusted_context: Option<&str>,
) -> Result<Plan, CoreError> {
    let lower = goal.to_ascii_lowercase();

    // Support Milestone 2 / Milestone 8 simulated prompt-injection / typosquat test triggers
    if let Some(untrusted) = untrusted_context {
        if untrusted.to_ascii_lowercase().contains("reqwests") {
            return Ok(Plan {
                plan_id: "plan-mock-injected-typosquat".to_string(),
                goal: goal.to_string(),
                summary: "Simulated LLM plan tainted by untrusted README requesting `reqwests`."
                    .to_string(),
                source: PlanSource::Llm {
                    model: "nim-mock-deterministic".to_string(),
                },
                steps: vec![PlannedStep {
                    id: "step-1".to_string(),
                    description: "Install HTTP library suggested in repository README".to_string(),
                    step: Step::InstallPackage {
                        ecosystem: Ecosystem::PyPI,
                        package_id: "reqwests".to_string(),
                        version: Some("2.32.0".to_string()),
                        source_adapter: AdapterKind::PyPI,
                    },
                }],
            });
        }
    }

    if lower.contains("node") || lower.contains("typescript") || lower.contains("react") {
        return Ok(Plan {
            plan_id: "plan-nim-node-ts".to_string(),
            goal: goal.to_string(),
            summary: "Set up Node.js LTS via Winget and install TypeScript via npm.".to_string(),
            source: PlanSource::Llm {
                model: "nim-mock-deterministic".to_string(),
            },
            steps: vec![
                PlannedStep {
                    id: "step-1".to_string(),
                    description: "Install OpenJS Node.js LTS runtime via Winget".to_string(),
                    step: Step::InstallPackage {
                        ecosystem: Ecosystem::Winget,
                        package_id: "OpenJS.NodeJS.LTS".to_string(),
                        version: Some("20.17.0".to_string()),
                        source_adapter: AdapterKind::Winget,
                    },
                },
                PlannedStep {
                    id: "step-2".to_string(),
                    description: "Install TypeScript compiler via npm".to_string(),
                    step: Step::InstallPackage {
                        ecosystem: Ecosystem::Npm,
                        package_id: "typescript".to_string(),
                        version: Some("5.6.2".to_string()),
                        source_adapter: AdapterKind::Npm,
                    },
                },
                PlannedStep {
                    id: "step-3".to_string(),
                    description: "Verify node --version".to_string(),
                    step: Step::RunVerifyCommand {
                        binary: AllowedBinary::Node,
                        args: vec!["--version".to_string()],
                    },
                },
            ],
        });
    }

    // Default fallback structured plan
    Ok(Plan {
        plan_id: "plan-nim-custom-env".to_string(),
        goal: goal.to_string(),
        summary: format!("AI-generated zero-trust setup plan for goal: '{goal}'"),
        source: PlanSource::Llm {
            model: "nim-mock-deterministic".to_string(),
        },
        steps: vec![
            PlannedStep {
                id: "step-1".to_string(),
                description: "Create isolated workspace directory".to_string(),
                step: Step::CreateIsolatedEnv {
                    kind: IsolationKind::IsolatedDirectory,
                    path: PathBuf::from("./.orchestrator-env"),
                },
            },
            PlannedStep {
                id: "step-2".to_string(),
                description: "Ensure Git is installed via Winget".to_string(),
                step: Step::InstallPackage {
                    ecosystem: Ecosystem::Winget,
                    package_id: "Git.Git".to_string(),
                    version: Some("2.46.0".to_string()),
                    source_adapter: AdapterKind::Winget,
                },
            },
            PlannedStep {
                id: "step-3".to_string(),
                description: "Verify git --version".to_string(),
                step: Step::RunVerifyCommand {
                    binary: AllowedBinary::Git,
                    args: vec!["--version".to_string()],
                },
            },
        ],
    })
}

/// Generates a deterministic mock remediation plan for testing the self-healing loop.
pub fn generate_mock_remediation_plan(
    goal: &str,
    _profile: &SystemProfile,
    failed_step_id: &str,
    diagnostic: &str,
    attempt: u8,
) -> Result<Plan, CoreError> {
    let lower_diag = diagnostic.to_ascii_lowercase();

    // Adversarial remediation simulation (tests Red blocking)
    if lower_diag.contains("evil") || lower_diag.contains("reqwests") {
        return Ok(Plan {
            plan_id: format!("plan-healer-malicious-{attempt}"),
            goal: goal.to_string(),
            summary: "Malicious remediation plan containing a typosquat package.".to_string(),
            source: PlanSource::Healer {
                failed_step_id: failed_step_id.to_string(),
                attempt,
            },
            steps: vec![PlannedStep {
                id: "step-heal-1".to_string(),
                description: "Install untrusted replacement".to_string(),
                step: Step::InstallPackage {
                    ecosystem: Ecosystem::PyPI,
                    package_id: "reqwests".to_string(),
                    version: Some("2.32.0".to_string()),
                    source_adapter: AdapterKind::PyPI,
                },
            }],
        });
    }

    // Standard valid zero-trust remediation
    Ok(Plan {
        plan_id: format!("plan-healer-{failed_step_id}-{attempt}"),
        goal: format!("Remediate failure at {failed_step_id}"),
        summary: format!(
            "AI-diagnosed remediation for {failed_step_id} (attempt {attempt}/2): refresh environment and verify binary."
        ),
        source: PlanSource::Healer {
            failed_step_id: failed_step_id.to_string(),
            attempt,
        },
        steps: vec![
            PlannedStep {
                id: "step-heal-1".to_string(),
                description: "Register tool directory on user PATH".to_string(),
                step: Step::AddToUserPath {
                    directory: PathBuf::from("C:\\Program Files\\LLVM\\bin"),
                },
            },
            PlannedStep {
                id: "step-heal-2".to_string(),
                description: "Verify clang++ responds after remediation".to_string(),
                step: Step::RunVerifyCommand {
                    binary: AllowedBinary::Clang,
                    args: vec!["--version".to_string()],
                },
            },
        ],
    })
}

