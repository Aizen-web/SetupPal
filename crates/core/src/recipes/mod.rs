use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::config::CoreError;
use crate::types::{Plan, PlanSource, PlannedStep};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub triggers: Vec<String>,
    pub summary: String,
    pub steps: Vec<PlannedStep>,
}

fn normalize_phrase(s: &str) -> String {
    s.to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

impl Recipe {
    /// A recipe matches only when the goal contains a full trigger phrase (or the recipe id/name),
    /// not a single fuzzy word. Single-product installs go through the winget resolver instead.
    pub fn matches_goal(&self, goal: &str) -> bool {
        let goal_n = normalize_phrase(goal);
        if goal_n.is_empty() {
            return false;
        }

        if goal_n == normalize_phrase(&self.id) || goal_n == normalize_phrase(&self.name) {
            return true;
        }

        for trigger in &self.triggers {
            let t = normalize_phrase(trigger);
            if t.is_empty() {
                continue;
            }
            // Exact phrase equality, or the trigger as a contiguous phrase inside the goal.
            if goal_n == t || goal_n.contains(&t) {
                return true;
            }
        }
        false
    }

    pub fn to_plan(&self, goal: &str) -> Plan {
        Plan {
            plan_id: format!("plan-recipe-{}", self.id),
            goal: goal.to_string(),
            summary: self.summary.clone(),
            source: PlanSource::Recipe {
                recipe_id: self.id.clone(),
            },
            steps: self.steps.clone(),
        }
    }
}

/// Every file in `recipes/`; the `embedded_manifests_cover_recipes_dir` test keeps this in sync.
const EMBEDDED_MANIFESTS: &[(&str, &str)] = &[
    ("cpp20_dev.yaml", include_str!("../../../../recipes/cpp20_dev.yaml")),
    ("everyday_apps.yaml", include_str!("../../../../recipes/everyday_apps.yaml")),
    ("minecraft_fabric_1_21.yaml", include_str!("../../../../recipes/minecraft_fabric_1_21.yaml")),
    ("obs_studio.yaml", include_str!("../../../../recipes/obs_studio.yaml")),
    ("python_ai_venv.yaml", include_str!("../../../../recipes/python_ai_venv.yaml")),
    ("rust_dev.yaml", include_str!("../../../../recipes/rust_dev.yaml")),
    ("web_fullstack_ts.yaml", include_str!("../../../../recipes/web_fullstack_ts.yaml")),
];

/// Manages curated YAML setup recipes with built-in embedded defaults
/// so the binary works even if invoked outside the repository working directory.
#[derive(Debug, Clone, Default)]
pub struct RecipeStore {
    recipes: Vec<Recipe>,
}

impl RecipeStore {
    pub fn load_from_dir_or_embedded(dir: &Path) -> Result<Self, CoreError> {
        let mut recipes = Vec::new();

        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
                    .unwrap_or(false)
                {
                    let raw = std::fs::read_to_string(&path)?;
                    let recipe: Recipe = serde_yaml::from_str(&raw)?;
                    recipes.push(recipe);
                }
            }
        }

        // Always merge embedded defaults if not already loaded
        let embedded = Self::embedded_recipes()?;
        for emb in embedded {
            if !recipes.iter().any(|r| r.id == emb.id) {
                recipes.push(emb);
            }
        }

        recipes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Self { recipes })
    }

    pub fn embedded_recipes() -> Result<Vec<Recipe>, CoreError> {
        let mut list = Vec::with_capacity(EMBEDDED_MANIFESTS.len());
        for (_, yaml) in EMBEDDED_MANIFESTS {
            list.push(serde_yaml::from_str::<Recipe>(yaml)?);
        }
        Ok(list)
    }

    pub fn find_matching(&self, goal: &str) -> Option<&Recipe> {
        self.recipes.iter().find(|r| r.matches_goal(goal))
    }

    pub fn list(&self) -> &[Recipe] {
        &self.recipes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::policy::{evaluate_step_policy, is_placeholder_sha256};
    use crate::types::{AdapterKind, Ecosystem, Step};

    fn recipes_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../recipes")
    }

    fn yaml_files() -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(recipes_dir())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".yaml") || n.ends_with(".yml"))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn embedded_manifests_cover_recipes_dir() {
        let embedded: Vec<String> = EMBEDDED_MANIFESTS.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(embedded, yaml_files());
        assert_eq!(RecipeStore::embedded_recipes().unwrap().len(), yaml_files().len());
    }

    #[test]
    fn every_recipe_file_parses_and_matches_embedded_copy() {
        let embedded = RecipeStore::embedded_recipes().unwrap();
        for name in yaml_files() {
            let raw = std::fs::read_to_string(recipes_dir().join(&name)).unwrap();
            let recipe: Recipe = serde_yaml::from_str(&raw).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(embedded.contains(&recipe), "{name} differs from its embedded copy");
        }
    }

    #[test]
    fn recipes_pass_policy_and_carry_no_placeholder_or_stale_pins() {
        for recipe in RecipeStore::embedded_recipes().unwrap() {
            let mut ids = std::collections::HashSet::new();
            for s in &recipe.steps {
                assert!(ids.insert(&s.id), "{}: duplicate step id {}", recipe.id, s.id);
                let violations = evaluate_step_policy(&s.step, &s.description);
                assert!(violations.is_empty(), "{} {}: {violations:?}", recipe.id, s.id);
                match &s.step {
                    Step::DownloadArtifact { expected_sha256: Some(h), .. } => {
                        assert!(!is_placeholder_sha256(h), "{} {}", recipe.id, s.id);
                    }
                    Step::InstallPackage {
                        ecosystem: Ecosystem::Winget,
                        version,
                        source_adapter: AdapterKind::Winget,
                        ..
                    } => assert!(version.is_none(), "{} {}: winget pins go stale", recipe.id, s.id),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn recipe_names_route_back_to_their_recipe() {
        let store = RecipeStore::load_from_dir_or_embedded(Path::new("no-such-dir")).unwrap();
        for r in store.list() {
            assert_eq!(store.find_matching(&r.name).map(|m| m.id.as_str()), Some(r.id.as_str()), "{}", r.name);
        }
    }

    #[test]
    fn game_app_requests_do_not_match_a_recipe() {
        let store = RecipeStore::load_from_dir_or_embedded(Path::new("no-such-dir")).unwrap();
        for goal in ["install Steam", "set up Roblox", "install Fortnite", "get the Epic Games launcher"] {
            assert!(store.find_matching(goal).is_none(), "{goal}");
        }
    }

    #[test]
    fn single_app_words_do_not_match_stack_recipes() {
        let store = RecipeStore::load_from_dir_or_embedded(Path::new("no-such-dir")).unwrap();
        for goal in [
            "install vlc",
            "install vlc please",
            "python 3.12",
            "typescript globally with npm",
            "install node",
            "install rust",
            "pip install requests",
        ] {
            assert!(
                store.find_matching(goal).is_none(),
                "{goal} must not match a multi-app recipe"
            );
        }
    }

    #[test]
    fn stack_phrases_match_their_recipes() {
        let store = RecipeStore::load_from_dir_or_embedded(Path::new("no-such-dir")).unwrap();
        for (goal, id) in [
            ("set me up for python data science", "python_ai_venv"),
            ("everyday apps", "everyday_apps"),
            ("7-Zip and VLC", "everyday_apps"),
            ("OBS Studio", "obs_studio"),
            ("set me up for rust", "rust_dev"),
            ("web dev setup", "web_fullstack_ts"),
            ("Modded Minecraft with Fabric", "minecraft_fabric_1_21"),
        ] {
            assert_eq!(store.find_matching(goal).map(|r| r.id.as_str()), Some(id), "{goal}");
        }
    }
}
