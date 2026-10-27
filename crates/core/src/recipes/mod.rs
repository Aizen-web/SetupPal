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

impl Recipe {
    /// Evaluates if the given goal matches this recipe using both exact substring checks
    /// and fuzzy phrase similarity (Jaro-Winkler).
    pub fn matches_goal(&self, goal: &str) -> bool {
        let lower_goal = goal.to_ascii_lowercase();

        // 1. Direct substring matching
        if self
            .triggers
            .iter()
            .any(|trigger| lower_goal.contains(&trigger.to_ascii_lowercase()))
        {
            return true;
        }

        // 2. Fuzzy trigger matching
        let words: Vec<&str> = lower_goal.split_whitespace().collect();
        for trigger in &self.triggers {
            let trigger_lower = trigger.to_ascii_lowercase();

            // Direct similarity between full goal and trigger
            if strsim::jaro_winkler(&lower_goal, &trigger_lower) > 0.85 {
                return true;
            }

            // Word-level fuzzy similarity
            for word in &words {
                if strsim::jaro_winkler(word, &trigger_lower) > 0.88 {
                    return true;
                }
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
        let raw_manifests = [
            include_str!("../../../../recipes/cpp20_dev.yaml"),
            include_str!("../../../../recipes/python_ai_venv.yaml"),
            include_str!("../../../../recipes/minecraft_fabric_1_21.yaml"),
        ];
        let mut list = Vec::with_capacity(raw_manifests.len());
        for yaml in raw_manifests {
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

    #[test]
    fn recipes_embedded_defaults_load_and_parse_all_flagships() {
        let store = RecipeStore::load_from_dir_or_embedded(Path::new("non_existent_dir"))
            .expect("should load embedded fallback");
        assert!(store.list().len() >= 3);

        let cpp = store.find_matching("set up c++20 development").expect("match cpp20");
        assert_eq!(cpp.id, "cpp20_dev");

        let python = store.find_matching("set me up with python ai and data science").expect("match python");
        assert_eq!(python.id, "python_ai_venv");

        let mc = store.find_matching("minecraft 1.21 fabric performance").expect("match minecraft");
        assert_eq!(mc.id, "minecraft_fabric_1_21");
    }

    #[test]
    fn recipes_fuzzy_matches_minor_typos() {
        let store = RecipeStore::load_from_dir_or_embedded(Path::new("non_existent_dir")).unwrap();
        // Exact match
        assert!(store.find_matching("python ai").is_some());
        // Minor typo match
        assert!(store.find_matching("pythn ai").is_some());
        assert!(store.find_matching("minecraf fabric").is_some());
    }
}
