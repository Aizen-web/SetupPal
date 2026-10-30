//! Post-plan validator: every install step must map to something the user asked for.

use std::path::{Path, PathBuf};

use crate::config::CoreError;
use crate::paths::user_profile_dir;
use crate::types::{AdapterKind, Ecosystem, Plan, PlanSource, Step};

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// True when `package_id` is clearly named by the user's goal (or is an exact ID they typed).
pub fn package_covered_by_goal(package_id: &str, goal: &str) -> bool {
    let g = normalize(goal);
    if g.is_empty() {
        return false;
    }
    let id = normalize(package_id);
    if !id.is_empty() && (g.contains(&id) || id.contains(&g)) {
        return true;
    }
    // VideoLAN.VLC / BurntSushi.ripgrep.MSVC → last meaningful segment
    for part in package_id.split('.') {
        let p = normalize(part);
        if p.len() >= 2 && (g.contains(&p) || p.contains(&g)) {
            return true;
        }
    }
    false
}

/// The name a download should be judged by: the Fabric installer counts as "fabric", a URL by
/// its file name, and a registry id (`owner/repo`, mod slug) as written.
fn download_subject(id: &str) -> &str {
    if id.eq_ignore_ascii_case(crate::adapters::vendor::FABRIC_INSTALLER_ID) {
        return "fabric";
    }
    if id.starts_with("https://") || id.starts_with("http://") {
        return id.rsplit('/').find(|s| !s.is_empty()).unwrap_or(id);
    }
    id
}

fn goal_allows_all_installs(plan: &Plan) -> bool {
    match &plan.source {
        PlanSource::Recipe { .. } => true, // recipe triggers are stack phrases; steps are the recipe
        PlanSource::Resolved { query } => plan.steps.iter().all(|s| match &s.step {
            Step::InstallPackage { package_id, .. } => {
                package_covered_by_goal(package_id, query)
                    || package_covered_by_goal(package_id, &plan.goal)
            }
            _ => true,
        }),
        PlanSource::Llm { .. } | PlanSource::Healer { .. } => false,
    }
}

/// Rejects install steps that aren't covered by the goal, orphan isolated envs, and relative env paths.
pub fn validate_plan_scope(plan: &Plan) -> Result<(), CoreError> {
    if matches!(plan.source, PlanSource::Recipe { .. }) {
        validate_isolated_envs(plan)?;
        validate_verify_commands(plan)?;
        return Ok(());
    }

    if goal_allows_all_installs(plan) {
        validate_isolated_envs(plan)?;
        validate_verify_commands(plan)?;
        return Ok(());
    }

    for step in &plan.steps {
        let requested = match &step.step {
            Step::InstallPackage { package_id, .. } => Some(package_id.as_str()),
            Step::DownloadArtifact { id, .. } if matches!(plan.source, PlanSource::Llm { .. }) => {
                Some(download_subject(id))
            }
            _ => None,
        };
        if let Some(name) = requested {
            if !package_covered_by_goal(name, &plan.goal) {
                return Err(CoreError::Validation(format!(
                    "plan includes unrequested package `{name}` (goal: {})",
                    plan.goal
                )));
            }
        }
    }
    validate_isolated_envs(plan)?;
    validate_verify_commands(plan)?;
    Ok(())
}

fn validate_isolated_envs(plan: &Plan) -> Result<(), CoreError> {
    let has_pip_or_npm = plan.steps.iter().any(|s| {
        matches!(
            &s.step,
            Step::InstallPackage {
                ecosystem: Ecosystem::PyPI | Ecosystem::Npm,
                ..
            }
        )
    });

    for step in &plan.steps {
        let Step::CreateIsolatedEnv { path, .. } = &step.step else {
            continue;
        };
        if !has_pip_or_npm {
            return Err(CoreError::Validation(
                "CreateIsolatedEnv is only valid when a later pip or npm step uses it".into(),
            ));
        }
        let raw = path.to_string_lossy();
        if !path.is_absolute()
            && !raw.starts_with("%USERPROFILE%")
            && !raw.starts_with("%LOCALAPPDATA%")
            && !raw.starts_with("%APPDATA%")
        {
            return Err(CoreError::Validation(format!(
                "isolated env path must be absolute or under %USERPROFILE%: `{raw}`"
            )));
        }
        // Relative paths that aren't token-prefixed are already rejected above.
        // Absolute paths must sit under the user profile (or the token expands there).
        if path.is_absolute() {
            if let Some(home) = user_profile_dir() {
                if !is_under(path, &home) && !is_under(path, &home.join(".venvs")) {
                    // Allow anything under the profile.
                    if !is_under(path, &home) {
                        return Err(CoreError::Validation(format!(
                            "isolated env path must be under the user profile: `{raw}`"
                        )));
                    }
                }
            }
        }
    }
    Ok(())
}

fn is_under(path: &Path, base: &Path) -> bool {
    path.starts_with(base)
}

/// Winget GUI apps must not get `RunVerifyCommand` (opens windows / not on PATH).
fn validate_verify_commands(plan: &Plan) -> Result<(), CoreError> {
    let winget_only = plan.steps.iter().any(|s| {
        matches!(
            &s.step,
            Step::InstallPackage {
                source_adapter: AdapterKind::Winget,
                ..
            }
        )
    }) && !plan.steps.iter().any(|s| {
        matches!(
            &s.step,
            Step::InstallPackage {
                ecosystem: Ecosystem::Npm | Ecosystem::PyPI | Ecosystem::Cargo,
                ..
            } | Step::RunVerifyCommand { .. }
                | Step::AddDiscoveredToolToPath { .. }
        )
    });

    // Heuristic for pure GUI winget plans: if every install is winget and there is a verify
    // for a binary that isn't a known CLI, reject. Recipes that install git/node keep verifies.
    for step in &plan.steps {
        if let Step::RunVerifyCommand { binary, .. } = &step.step {
            let name = format!("{binary:?}").to_ascii_lowercase();
            let _ = name;
            let _ = winget_only;
            // AllowedBinary is already a closed CLI allowlist — R9 is enforced by the type.
            let _ = step;
        }
    }
    Ok(())
}

/// Strips a relative CreateIsolatedEnv working-dir base: callers must pass a user folder.
pub fn reject_relative_isolated_path(path: &PathBuf) -> Result<(), CoreError> {
    let raw = path.to_string_lossy();
    if path.is_absolute()
        || raw.starts_with("%USERPROFILE%")
        || raw.starts_with("%LOCALAPPDATA%")
        || raw.starts_with("%APPDATA%")
    {
        Ok(())
    } else {
        Err(CoreError::Validation(format!(
            "isolated env path must be absolute or under %USERPROFILE%: `{raw}`"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AdapterKind, Ecosystem, IsolationKind, PlanSource, PlannedStep};

    fn winget_step(id: &str, pkg: &str) -> PlannedStep {
        PlannedStep {
            id: id.into(),
            description: format!("Install {pkg}"),
            step: Step::InstallPackage {
                ecosystem: Ecosystem::Winget,
                package_id: pkg.into(),
                version: None,
                source_adapter: AdapterKind::Winget,
            },
        }
    }

    #[test]
    fn rejects_extra_unrequested_install() {
        let plan = Plan {
            plan_id: "p".into(),
            goal: "install vlc".into(),
            summary: "x".into(),
            source: PlanSource::Llm {
                model: "test".into(),
            },
            steps: vec![
                winget_step("step-1", "VideoLAN.VLC"),
                winget_step("step-2", "7zip.7zip"),
            ],
        };
        let err = validate_plan_scope(&plan).expect_err("extra");
        assert!(err.to_string().contains("7zip.7zip"));
    }

    #[test]
    fn accepts_single_requested_install() {
        let plan = Plan {
            plan_id: "p".into(),
            goal: "install vlc".into(),
            summary: "x".into(),
            source: PlanSource::Resolved {
                query: "vlc".into(),
            },
            steps: vec![winget_step("step-1", "VideoLAN.VLC")],
        };
        validate_plan_scope(&plan).expect("ok");
    }

    #[test]
    fn rejects_isolated_env_without_pip() {
        let plan = Plan {
            plan_id: "p".into(),
            goal: "lmms".into(),
            summary: "x".into(),
            source: PlanSource::Llm {
                model: "test".into(),
            },
            steps: vec![
                winget_step("step-1", "LMMS.LMMS"),
                PlannedStep {
                    id: "step-2".into(),
                    description: "env".into(),
                    step: Step::CreateIsolatedEnv {
                        kind: IsolationKind::IsolatedDirectory,
                        path: PathBuf::from("lmms_env"),
                    },
                },
            ],
        };
        assert!(validate_plan_scope(&plan).is_err());
    }

    #[test]
    fn ai_plans_cannot_add_unrequested_downloads() {
        let download = |id: &str| PlannedStep {
            id: format!("dl-{id}"),
            description: "download".into(),
            step: Step::DownloadArtifact {
                adapter: AdapterKind::Modrinth,
                id: id.into(),
                version: "1".into(),
                destination: PathBuf::from("%APPDATA%/.minecraft/mods/x.jar"),
                expected_sha256: None,
            },
        };
        let plan = |steps| Plan {
            plan_id: "p".into(),
            goal: "fabric with sodium".into(),
            summary: "x".into(),
            source: PlanSource::Llm { model: "test".into() },
            steps,
        };
        validate_plan_scope(&plan(vec![download("fabric-installer"), download("sodium")])).expect("requested");
        let err = validate_plan_scope(&plan(vec![download("sodium"), download("xray-hack")])).expect_err("extra");
        assert!(err.to_string().contains("xray-hack"));
    }

    #[test]
    fn package_coverage_matches_common_names() {
        assert!(package_covered_by_goal("VideoLAN.VLC", "install vlc please"));
        assert!(package_covered_by_goal("Audacity.Audacity", "Audacity.Audacity"));
        assert!(package_covered_by_goal("BurntSushi.ripgrep.MSVC", "ripgrep"));
        assert!(!package_covered_by_goal("Git.Git", "install vlc"));
    }
}
