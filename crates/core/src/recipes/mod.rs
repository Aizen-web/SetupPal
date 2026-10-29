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
