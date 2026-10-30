//! Deterministic answer to "can SetupPal install this unattended from the command line?".

use serde::{Deserialize, Serialize};

use crate::adapters::github::winget_id_for_repo;
use crate::adapters::winget::{parse_winget_show, winget_show_args, WingetShowInfo};
use crate::os::OsProvider;
use crate::recipes::RecipeStore;
use crate::safety::{PlanSafetyReport, RiskRating};
use crate::types::{AdapterKind, Ecosystem, Plan, Step};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Installability {
    CliInstallable,
    NeedsAdmin { reason: String },
    ManualOnly { reason: ManualReason, detail: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManualReason {
    StoreOnly,
    NoSilentInstaller,
    Unknown,
}

impl ManualReason {
    pub fn user_text(self) -> &'static str {
        match self {
            ManualReason::StoreOnly => "This app is only offered through the Microsoft Store.",
            ManualReason::NoSilentInstaller => "This app has no installer that can run unattended.",
            ManualReason::Unknown => "SetupPal could not confirm an unattended install path for this app.",
        }
    }
}

/// Result for one plan step, plus non-blocking cautions (shown as Yellow reasons).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepInstallability {
    pub step_id: String,
    pub item: String,
    #[serde(flatten)]
    pub installability: Installability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cautions: Vec<String>,
}

pub const INSTALLER_TYPE_UNKNOWN_REASON: &str = "Installer type unknown: SetupPal could not read this package's winget manifest";
pub const ADMIN_REASON: &str = "Administrator access needed: this package only installs machine-wide";

/// Installer types winget runs unattended. Community-repo manifests of type `exe` must declare
/// silent switches to be accepted, so `exe` is included.
const SILENT_INSTALLER_TYPES: &[&str] = &["exe", "msi", "msix", "appx", "wix", "inno", "nullsoft", "burn", "portable", "zip"];

/// Microsoft Store product IDs: 12 characters starting with `9` or `XP`.
pub fn is_store_product_id(id: &str) -> bool {
    let id = id.trim();
    id.len() == 12
        && id.chars().all(|c| c.is_ascii_alphanumeric())
        && (id.starts_with('9') || id.to_ascii_uppercase().starts_with("XP"))
}

/// Classifies a winget package from its ID and parsed `winget show` output. Whether the app
/// later asks for a sign-in (Steam, Discord, Roblox) does not matter; only the installer does.
pub fn classify_winget(id: &str, info: Option<&WingetShowInfo>) -> (Installability, Vec<String>) {
    if is_store_product_id(id) {
        return (
            Installability::ManualOnly {
                reason: ManualReason::StoreOnly,
                detail: format!("{id} is a Microsoft Store product."),
            },
            Vec::new(),
        );
    }
    let Some(info) = info else {
        return (Installability::CliInstallable, vec![INSTALLER_TYPE_UNKNOWN_REASON.to_string()]);
    };
    let installer_type = info.installer_type.as_deref().map(str::to_ascii_lowercase);
    let display = info.name.clone().unwrap_or_else(|| id.to_string());

    let mut verdict = match installer_type.as_deref() {
        Some("msstore") => Installability::ManualOnly {
            reason: ManualReason::StoreOnly,
            detail: format!("{display} is only published through the Microsoft Store."),
        },
        Some(t) if SILENT_INSTALLER_TYPES.contains(&t) => Installability::CliInstallable,
        Some(_) | None => {
            return (Installability::CliInstallable, vec![INSTALLER_TYPE_UNKNOWN_REASON.to_string()]);
        }
    };

    if verdict == Installability::CliInstallable
        && info
            .installer_scope
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("machine"))
    {
        verdict = Installability::NeedsAdmin {
            reason: ADMIN_REASON.to_string(),
        };
    }
    (verdict, Vec::new())
}

pub(crate) async fn winget_show(os: &dyn OsProvider, id: &str, version: Option<&str>) -> Option<WingetShowInfo> {
    let paths = os.refresh_and_get_path().await.ok()?;
    let winget = os.which_in_paths("winget", &paths)?;
    let out = os
        .run_command(&winget, &winget_show_args(id, version), &paths)
        .await
        .ok()?;
    if out.exit_code != Some(0) {
        return None;
    }
    Some(parse_winget_show(&out.stdout))
}

async fn check_winget(os: &dyn OsProvider, id: &str, version: Option<&str>) -> (Installability, Vec<String>) {
    if is_store_product_id(id) {
        return classify_winget(id, None);
    }
    let info = winget_show(os, id, version).await;
    classify_winget(id, info.as_ref())
}

/// Checks one step, returning the verdict and any cautions.
pub async fn check_detailed(step: &Step, os: &dyn OsProvider) -> (Installability, Vec<String>) {
    match step {
        Step::InstallPackage {
            ecosystem,
            package_id,
            version,
            source_adapter,
        } => match (ecosystem, source_adapter) {
            (Ecosystem::Winget, _) | (_, AdapterKind::Winget) => {
                check_winget(os, package_id, version.as_deref()).await
            }
            (Ecosystem::GitHub, _) | (_, AdapterKind::GitHubReleases) => match winget_id_for_repo(package_id) {
                Some(winget_id) => check_winget(os, winget_id, None).await,
                None => (
                    Installability::CliInstallable,
                    vec![format!(
                        "{package_id}: the release asset type is checked at install time; a setup .exe falls back to a manual guide"
                    )],
                ),
            },
            (Ecosystem::Vendor, _) | (_, AdapterKind::OfficialVendorUrl) => (
                Installability::ManualOnly {
                    reason: ManualReason::NoSilentInstaller,
                    detail: format!("{package_id} is a vendor download without an unattended install path."),
                },
                Vec::new(),
            ),
            _ => (Installability::CliInstallable, Vec::new()),
        },
        _ => (Installability::CliInstallable, Vec::new()),
    }
}

pub async fn check(step: &Step, os: &dyn OsProvider) -> Installability {
    check_detailed(step, os).await.0
}

fn step_item(step: &Step) -> String {
    match step {
        Step::InstallPackage { package_id, .. } => package_id.clone(),
        Step::DownloadArtifact { id, .. } => id.clone(),
        other => format!("{other:?}").split([' ', '{']).next().unwrap_or_default().to_string(),
    }
}

/// Checks every step of a plan.
pub async fn check_plan(plan: &Plan, os: &dyn OsProvider) -> Vec<StepInstallability> {
    let mut out = Vec::with_capacity(plan.steps.len());
    for s in &plan.steps {
        let (installability, cautions) = check_detailed(&s.step, os).await;
        out.push(StepInstallability {
            step_id: s.id.clone(),
            item: step_item(&s.step),
            installability,
            cautions,
        });
    }
    out
}

/// First `ManualOnly` step, if the plan cannot run unattended.
pub fn first_manual(checks: &[StepInstallability]) -> Option<&StepInstallability> {
    checks
        .iter()
        .find(|c| matches!(c.installability, Installability::ManualOnly { .. }))
}

/// Folds installability into the safety report: `NeedsAdmin` steps and cautions become
/// Yellow, `ManualOnly` steps become Red so they can never run.
pub fn apply_to_safety_report(checks: &[StepInstallability], report: &mut PlanSafetyReport) {
    for c in checks {
        match &c.installability {
            Installability::CliInstallable => {}
            Installability::NeedsAdmin { reason } => {
                report.escalate_step(&c.step_id, RiskRating::Yellow, reason.clone())
            }
            Installability::ManualOnly { reason, detail } => report.escalate_step(
                &c.step_id,
                RiskRating::Red,
                format!("{} {detail}", reason.user_text()),
            ),
        }
        for caution in &c.cautions {
            report.escalate_step(&c.step_id, RiskRating::Yellow, caution.clone());
        }
    }
}

/// Reply for apps SetupPal cannot install unattended.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManualGuide {
    pub kind: String,
    pub item: String,
    pub reason: ManualReason,
    pub detail: String,
    pub steps: Vec<String>,
    /// Goals SetupPal can run instead; each matches an embedded recipe.
    pub alternatives: Vec<String>,
}

const MAX_ALTERNATIVES: usize = 5;

/// Recipe IDs suggested in place of a manual-only app.
const ALTERNATIVE_RECIPE_IDS: &[&str] = &["everyday_apps", "obs_studio", "web_fullstack_ts", "python_ai_venv", "cpp20_dev"];

/// Builds a manual setup guide; alternatives are names of recipes present in `recipes`.
pub fn manual_guide(item: &str, reason: ManualReason, detail: &str, recipes: &RecipeStore) -> ManualGuide {
    let steps = match reason {
        ManualReason::StoreOnly => vec![
            "Open the Microsoft Store app.".to_string(),
            format!("Search for {item} and press Get or Install."),
            "Sign in with your Microsoft account if the Store asks.".to_string(),
        ],
        ManualReason::NoSilentInstaller | ManualReason::Unknown => vec![
            format!("Find {item} on its official website."),
            format!("Download the {item} installer and run it."),
            "Follow the installer's prompts.".to_string(),
        ],
    };

    let alternatives = ALTERNATIVE_RECIPE_IDS
        .iter()
        .filter_map(|id| recipes.list().iter().find(|r| r.id == *id))
        .map(|r| r.name.clone())
        .take(MAX_ALTERNATIVES)
        .collect();

    ManualGuide {
        kind: "manual".to_string(),
        item: item.to_string(),
        reason,
        detail: detail.to_string(),
        steps,
        alternatives,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::mock::{ok, MockOsProvider};
    use crate::types::PlannedStep;

    fn show(installer_type: &str, scope: Option<&str>) -> String {
        let mut s = format!(
            "Found Example App [Example.App]\nVersion: 1.2.3\nPublisher: Example\nInstaller:\n  Installer Type: {installer_type}\n"
        );
        if let Some(scope) = scope {
            s.push_str(&format!("  Installer Scope: {scope}\n"));
        }
        s.push_str("  Installer Url: https://example.com/app.bin\n  Installer SHA256: 2f1c\n");
        s
    }

    fn winget_step(id: &str) -> Step {
        Step::InstallPackage {
            ecosystem: Ecosystem::Winget,
            package_id: id.to_string(),
            version: None,
            source_adapter: AdapterKind::Winget,
        }
    }

    fn os_with_show(stdout: String) -> MockOsProvider {
        MockOsProvider::new()
            .with_binary("winget", r"C:\winget.exe")
            .on("winget", &["show"], ok(&stdout))
    }

    #[tokio::test]
    async fn silent_installer_types_are_cli_installable() {
        for t in ["msi", "msix", "wix", "inno", "nullsoft", "burn", "portable (zip)", "zip"] {
            let os = os_with_show(show(t, None));
            assert_eq!(check(&winget_step("Example.App"), &os).await, Installability::CliInstallable, "{t}");
        }
    }

    #[tokio::test]
    async fn exe_installers_from_winget_are_cli_installable() {
        for id in ["Discord.Discord", "Spotify.Spotify", "Rustlang.Rustup"] {
            let os = os_with_show(show("exe", None));
            assert_eq!(check(&winget_step(id), &os).await, Installability::CliInstallable, "{id}");
        }
    }

    #[tokio::test]
    async fn msstore_type_is_store_only() {
        let os = os_with_show(show("msstore", None));
        assert!(matches!(
            check(&winget_step("Example.App"), &os).await,
            Installability::ManualOnly { reason: ManualReason::StoreOnly, .. }
        ));
        assert!(matches!(
            check(&winget_step("9WZDNCRFJ3TJ"), &MockOsProvider::new()).await,
            Installability::ManualOnly { reason: ManualReason::StoreOnly, .. }
        ));
    }

    #[tokio::test]
    async fn machine_scope_needs_admin() {
        let os = os_with_show(show("wix", Some("machine")));
        assert!(matches!(
            check(&winget_step("Example.App"), &os).await,
            Installability::NeedsAdmin { .. }
        ));
        let os = os_with_show(show("wix", Some("user")));
        assert_eq!(check(&winget_step("Example.App"), &os).await, Installability::CliInstallable);
    }

    #[tokio::test]
    async fn unparseable_output_is_installable_with_caution() {
        let os = os_with_show("garbage".to_string());
        let (verdict, cautions) = check_detailed(&winget_step("Example.App"), &os).await;
        assert_eq!(verdict, Installability::CliInstallable);
        assert_eq!(cautions, vec![INSTALLER_TYPE_UNKNOWN_REASON.to_string()]);
    }

    #[tokio::test]
    async fn game_launchers_on_winget_are_cli_installable() {
        for (id, installer_type) in [
            ("Valve.Steam", "nullsoft"),
            ("EpicGames.EpicGamesLauncher", "wix"),
            ("Mojang.MinecraftLauncher", "wix"),
            ("Roblox.Roblox", "portable"),
        ] {
            let os = os_with_show(show(installer_type, None));
            assert_eq!(check(&winget_step(id), &os).await, Installability::CliInstallable, "{id}");
            assert!(!os.calls().is_empty(), "{id} must be judged from its winget manifest");
        }
    }

    #[tokio::test]
    async fn language_packages_are_cli_installable() {
        let os = MockOsProvider::new();
        for (eco, adapter) in [
            (Ecosystem::Npm, AdapterKind::Npm),
            (Ecosystem::PyPI, AdapterKind::PyPI),
            (Ecosystem::Cargo, AdapterKind::Cargo),
            (Ecosystem::Modrinth, AdapterKind::Modrinth),
        ] {
            let step = Step::InstallPackage {
                ecosystem: eco,
                package_id: "steam-user".to_string(),
                version: None,
                source_adapter: adapter,
            };
            assert_eq!(check(&step, &os).await, Installability::CliInstallable);
        }
    }

    #[tokio::test]
    async fn manual_steps_escalate_to_red_and_guide_lists_recipes() {
        let plan = Plan {
            plan_id: "p".to_string(),
            goal: "install minecraft bedrock".to_string(),
            summary: String::new(),
            source: crate::types::PlanSource::Llm {
                model: "test".to_string(),
            },
            steps: vec![PlannedStep {
                id: "s1".to_string(),
                description: "Install Minecraft Bedrock".to_string(),
                step: winget_step("9NBLGGH2JHXJ"),
            }],
        };
        let checks = check_plan(&plan, &MockOsProvider::new()).await;
        let manual = first_manual(&checks).cloned();
        let Some(manual) = manual else {
            panic!("a Store-only product should be manual");
        };
        assert_eq!(manual.item, "9NBLGGH2JHXJ");

        let recipes = RecipeStore::load_from_dir_or_embedded(std::path::Path::new("missing-dir")).unwrap();
        let Installability::ManualOnly { reason, detail } = &manual.installability else {
            panic!("expected manual");
        };
        assert_eq!(*reason, ManualReason::StoreOnly);
        let guide = manual_guide(&manual.item, *reason, detail, &recipes);
        assert_eq!(guide.kind, "manual");
        assert!(guide.steps[0].contains("Microsoft Store"));
        assert_eq!(guide.alternatives.len(), ALTERNATIVE_RECIPE_IDS.len());
        for alt in &guide.alternatives {
            let matched = recipes.find_matching(alt).map(|r| r.name.clone());
            assert_eq!(matched.as_ref(), Some(alt), "alternative `{alt}` must route back to its recipe");
        }
    }

    #[test]
    fn every_alternative_recipe_exists() {
        let recipes = RecipeStore::embedded_recipes().unwrap();
        for id in ALTERNATIVE_RECIPE_IDS {
            assert!(recipes.iter().any(|r| r.id == *id), "missing recipe {id}");
        }
    }
}
