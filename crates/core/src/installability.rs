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
    LauncherOrDrm,
    StoreOnly,
    NoSilentInstaller,
    InteractiveLogin,
    Unknown,
}

impl ManualReason {
    pub fn user_text(self) -> &'static str {
        match self {
            ManualReason::LauncherOrDrm => "This app installs through its own launcher or DRM and needs you to sign in.",
            ManualReason::StoreOnly => "This app is only offered through the Microsoft Store.",
            ManualReason::NoSilentInstaller => "This app has no installer that can run unattended.",
            ManualReason::InteractiveLogin => "This app needs an interactive account sign-in to install.",
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

struct Launcher {
    name: &'static str,
    winget_ids: &'static [&'static str],
    winget_prefixes: &'static [&'static str],
    keywords: &'static [&'static str],
    official_url: &'static str,
}

const LAUNCHERS: &[Launcher] = &[
    Launcher {
        name: "Steam",
        winget_ids: &["valve.steam"],
        winget_prefixes: &[],
        keywords: &["steam"],
        official_url: "https://store.steampowered.com/about/",
    },
    Launcher {
        name: "Epic Games Launcher",
        winget_ids: &["epicgames.epicgameslauncher"],
        winget_prefixes: &[],
        keywords: &["epic games", "epic launcher", "fortnite", "unreal engine"],
        official_url: "https://store.epicgames.com/download",
    },
    Launcher {
        name: "Riot Client",
        winget_ids: &[],
        winget_prefixes: &["riotgames."],
        keywords: &["riot client", "riot games", "valorant", "league of legends"],
        official_url: "https://www.riotgames.com/",
    },
    Launcher {
        name: "EA app",
        winget_ids: &["electronicarts.eadesktop", "electronicarts.origin"],
        winget_prefixes: &[],
        keywords: &["ea app", "ea desktop", "origin launcher"],
        official_url: "https://www.ea.com/ea-app",
    },
    Launcher {
        name: "Ubisoft Connect",
        winget_ids: &["ubisoft.connect"],
        winget_prefixes: &[],
        keywords: &["ubisoft connect", "uplay"],
        official_url: "https://ubisoftconnect.com/",
    },
    Launcher {
        name: "Minecraft Launcher",
        winget_ids: &["mojang.minecraftlauncher", "9pgw18npbzv5"],
        winget_prefixes: &[],
        keywords: &["minecraft launcher"],
        official_url: "https://www.minecraft.net/download",
    },
    Launcher {
        name: "Minecraft Bedrock Edition",
        winget_ids: &["9nblggh2jhxj"],
        winget_prefixes: &[],
        keywords: &["minecraft bedrock", "bedrock edition", "minecraft for windows"],
        official_url: "https://www.minecraft.net/download",
    },
    Launcher {
        name: "Roblox",
        winget_ids: &["roblox.roblox", "roblox.robloxstudio"],
        winget_prefixes: &[],
        keywords: &["roblox"],
        official_url: "https://www.roblox.com/download",
    },
    Launcher {
        name: "Battle.net",
        winget_ids: &["blizzard.battlenet"],
        winget_prefixes: &[],
        keywords: &["battle.net", "battlenet", "blizzard"],
        official_url: "https://download.battle.net/",
    },
    Launcher {
        name: "GOG Galaxy",
        winget_ids: &["gog.galaxy"],
        winget_prefixes: &[],
        keywords: &["gog galaxy"],
        official_url: "https://www.gog.com/galaxy",
    },
];

/// winget packages of installer type `exe` whose manifests declare silent switches.
const SILENT_EXE_IDS: &[&str] = &[
    "rustlang.rustup",
    "mozilla.firefox",
    "google.chrome",
    "docker.dockerdesktop",
    "microsoft.visualstudio.2022.buildtools",
    "microsoft.visualstudio.2022.community",
];

const SILENT_INSTALLER_TYPES: &[&str] = &["msi", "msix", "appx", "wix", "inno", "nullsoft", "burn", "portable", "zip"];

fn contains_word(haystack: &str, needle: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        let begin = start + pos;
        let end = begin + needle.len();
        let before_ok = haystack[..begin]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_ascii_alphanumeric());
        let after_ok = haystack[end..].chars().next().is_none_or(|c| !c.is_ascii_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        start = begin + needle.len().max(1);
    }
    false
}

fn launcher_for_winget_id(id: &str) -> Option<&'static Launcher> {
    let id = id.trim().to_ascii_lowercase();
    LAUNCHERS.iter().find(|l| {
        l.winget_ids.contains(&id.as_str()) || l.winget_prefixes.iter().any(|p| id.starts_with(p))
    })
}

/// Name of the launcher or DRM product mentioned in free text or a package ID, if any.
pub fn launcher_or_drm_match(text: &str) -> Option<&'static str> {
    if let Some(l) = launcher_for_winget_id(text) {
        return Some(l.name);
    }
    let lower = text.to_ascii_lowercase();
    LAUNCHERS
        .iter()
        .find(|l| l.keywords.iter().any(|k| contains_word(&lower, k)))
        .map(|l| l.name)
}

/// Official download page for a launcher returned by `launcher_or_drm_match`.
pub fn launcher_official_url(name: &str) -> Option<&'static str> {
    LAUNCHERS.iter().find(|l| l.name == name).map(|l| l.official_url)
}

/// Microsoft Store product IDs: 12 characters starting with `9` or `XP`.
pub fn is_store_product_id(id: &str) -> bool {
    let id = id.trim();
    id.len() == 12
        && id.chars().all(|c| c.is_ascii_alphanumeric())
        && (id.starts_with('9') || id.to_ascii_uppercase().starts_with("XP"))
}

/// Classifies a winget package from its ID and parsed `winget show` output.
pub fn classify_winget(id: &str, info: Option<&WingetShowInfo>) -> (Installability, Vec<String>) {
    if let Some(l) = launcher_for_winget_id(id) {
        return (
            Installability::ManualOnly {
                reason: ManualReason::LauncherOrDrm,
                detail: format!("{} is a game launcher that needs your account to install and run.", l.name),
            },
            Vec::new(),
        );
    }
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
        Some("exe") if !SILENT_EXE_IDS.contains(&id.to_ascii_lowercase().as_str()) => Installability::ManualOnly {
            reason: ManualReason::NoSilentInstaller,
            detail: format!("{display} ships a custom .exe installer with no known silent mode."),
        },
        Some(t) if t == "exe" || SILENT_INSTALLER_TYPES.contains(&t) => Installability::CliInstallable,
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

async fn winget_show(os: &dyn OsProvider, id: &str, version: Option<&str>) -> Option<WingetShowInfo> {
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
    if launcher_for_winget_id(id).is_some() || is_store_product_id(id) {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub official_url: Option<String>,
    pub steps: Vec<String>,
    /// Goals SetupPal can run instead; each matches an embedded recipe.
    pub alternatives: Vec<String>,
}

const MAX_ALTERNATIVES: usize = 5;

/// Recipe IDs suggested in place of a manual-only app, by reason.
fn alternative_recipe_ids(reason: ManualReason) -> &'static [&'static str] {
    match reason {
        ManualReason::LauncherOrDrm => &["minecraft_fabric_1_21", "obs_studio", "everyday_apps"],
        ManualReason::StoreOnly
        | ManualReason::NoSilentInstaller
        | ManualReason::InteractiveLogin
        | ManualReason::Unknown => &["everyday_apps", "obs_studio", "web_fullstack_ts", "python_ai_venv", "cpp20_dev"],
    }
}

/// Builds a manual setup guide; alternatives are names of recipes present in `recipes`.
pub fn manual_guide(item: &str, reason: ManualReason, detail: &str, recipes: &RecipeStore) -> ManualGuide {
    let launcher = launcher_or_drm_match(item);
    let official_url = launcher.and_then(launcher_official_url).map(str::to_string);
    let display = launcher.unwrap_or(item);

    let mut steps = Vec::new();
    match &official_url {
        Some(url) => steps.push(format!("Open {url} in your browser.")),
        None => steps.push(format!("Find {display} on its official website.")),
    }
    steps.push(format!("Download the {display} installer and run it."));
    steps.push("Follow the installer's prompts and sign in if it asks for an account.".to_string());

    let alternatives = alternative_recipe_ids(reason)
        .iter()
        .filter_map(|id| recipes.list().iter().find(|r| r.id == *id))
        .map(|r| r.name.clone())
        .take(MAX_ALTERNATIVES)
        .collect();

    ManualGuide {
        kind: "manual".to_string(),
        item: display.to_string(),
        reason,
        detail: detail.to_string(),
        official_url,
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
    async fn unknown_exe_has_no_silent_installer() {
        let os = os_with_show(show("exe", None));
        assert!(matches!(
            check(&winget_step("Example.App"), &os).await,
            Installability::ManualOnly { reason: ManualReason::NoSilentInstaller, .. }
        ));
    }

    #[tokio::test]
    async fn known_silent_exe_is_cli_installable() {
        let os = os_with_show(show("exe", None));
        assert_eq!(check(&winget_step("Rustlang.Rustup"), &os).await, Installability::CliInstallable);
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
    async fn steam_is_always_manual_even_if_winget_says_nullsoft() {
        let os = os_with_show(show("nullsoft", None));
        assert!(matches!(
            check(&winget_step("Valve.Steam"), &os).await,
            Installability::ManualOnly { reason: ManualReason::LauncherOrDrm, .. }
        ));
        assert!(os.calls().is_empty());
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

    #[test]
    fn launcher_keywords_match_words_not_substrings() {
        assert_eq!(launcher_or_drm_match("please install Steam"), Some("Steam"));
        assert_eq!(launcher_or_drm_match("RiotGames.Valorant.EU"), Some("Riot Client"));
        assert_eq!(launcher_or_drm_match("Fortnite"), Some("Epic Games Launcher"));
        assert_eq!(launcher_or_drm_match("set up a steamroller"), None);
        assert_eq!(launcher_or_drm_match("Modded Minecraft Java with Fabric"), None);
    }

    #[tokio::test]
    async fn manual_steps_escalate_to_red_and_guide_lists_recipes() {
        let plan = Plan {
            plan_id: "p".to_string(),
            goal: "install steam".to_string(),
            summary: String::new(),
            source: crate::types::PlanSource::Llm {
                model: "test".to_string(),
            },
            steps: vec![PlannedStep {
                id: "s1".to_string(),
                description: "Install Steam".to_string(),
                step: winget_step("Valve.Steam"),
            }],
        };
        let checks = check_plan(&plan, &MockOsProvider::new()).await;
        let manual = first_manual(&checks).cloned();
        let Some(manual) = manual else {
            panic!("Steam should be manual");
        };
        assert_eq!(manual.item, "Valve.Steam");

        let recipes = RecipeStore::load_from_dir_or_embedded(std::path::Path::new("missing-dir")).unwrap();
        let Installability::ManualOnly { reason, detail } = &manual.installability else {
            panic!("expected manual");
        };
        let guide = manual_guide(&manual.item, *reason, detail, &recipes);
        assert_eq!(guide.kind, "manual");
        assert_eq!(guide.item, "Steam");
        assert_eq!(guide.official_url.as_deref(), Some("https://store.steampowered.com/about/"));
        assert_eq!(guide.alternatives.len(), alternative_recipe_ids(*reason).len());
        for alt in &guide.alternatives {
            let matched = recipes.find_matching(alt).map(|r| r.name.clone());
            assert_eq!(matched.as_ref(), Some(alt), "alternative `{alt}` must route back to its recipe");
        }
    }

    #[test]
    fn every_alternative_recipe_exists() {
        let recipes = RecipeStore::embedded_recipes().unwrap();
        for reason in [
            ManualReason::LauncherOrDrm,
            ManualReason::StoreOnly,
            ManualReason::NoSilentInstaller,
            ManualReason::InteractiveLogin,
            ManualReason::Unknown,
        ] {
            for id in alternative_recipe_ids(reason) {
                assert!(recipes.iter().any(|r| r.id == *id), "missing recipe {id}");
            }
        }
    }
}
