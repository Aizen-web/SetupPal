use std::sync::Arc;

use crate::config::CoreError;
use crate::events::{AgentEvent, EventBus};
use crate::llm::LlmClient;
use crate::recipes::RecipeStore;
use crate::types::{Plan, SystemProfile};

/// Converts `Goal + SystemProfile + Optional<UntrustedContext>` into a validated `Plan`.
/// Checks deterministic `RecipeStore` first; falls back to `LlmClient` (NVIDIA NIM) if no
/// recipe matches or if external context requires custom synthesis.
pub struct Planner {
    recipes: RecipeStore,
    llm: Arc<dyn LlmClient>,
    events: EventBus,
}

impl Planner {
    pub fn new(recipes: RecipeStore, llm: Arc<dyn LlmClient>, events: EventBus) -> Self {
        Self {
            recipes,
            llm,
            events,
        }
    }

    pub async fn create_plan(
        &self,
        goal: &str,
        profile: &SystemProfile,
        untrusted_context: Option<&str>,
    ) -> Result<Plan, CoreError> {
        let trimmed_goal = goal.trim();
        if trimmed_goal.is_empty() {
            return Err(CoreError::Validation("goal must not be empty".to_string()));
        }

        self.events.emit(AgentEvent::PlanningStarted {
            goal: trimmed_goal.to_string(),
        });

        // 1. Check deterministic recipes first (when no untrusted repo context overrides it)
        if untrusted_context.is_none() {
            if let Some(recipe) = self.recipes.find_matching(trimmed_goal) {
                self.events.emit(AgentEvent::RecipeMatched {
                    recipe_id: recipe.id.clone(),
                });
                let plan = recipe.to_plan(trimmed_goal);
                self.validate_plan_structure(&plan)?;
                self.events.emit(AgentEvent::PlanGenerated {
                    plan: Box::new(plan.clone()),
                });
                return Ok(plan);
            }
        }

        // 2. Fall back to NVIDIA NIM with JSON Schema enforcement
        self.events.emit(AgentEvent::LlmPlanningFallback {
            goal: trimmed_goal.to_string(),
            model: self.llm.model_name().to_string(),
        });

        let plan = self
            .llm
            .propose_plan(trimmed_goal, profile, untrusted_context)
            .await?;

        self.validate_plan_structure(&plan)?;

        self.events.emit(AgentEvent::PlanGenerated {
            plan: Box::new(plan.clone()),
        });

        Ok(plan)
    }

    /// Ensures the plan has at least one step and unique, non-empty step IDs.
    pub fn validate_plan_structure(&self, plan: &Plan) -> Result<(), CoreError> {
        if plan.steps.is_empty() {
            return Err(CoreError::Validation(
                "generated plan contains zero steps".to_string(),
            ));
        }
        let mut ids = std::collections::HashSet::new();
        for step in &plan.steps {
            if step.id.trim().is_empty() {
                return Err(CoreError::Validation(
                    "planned step has an empty id".to_string(),
                ));
            }
            if !ids.insert(step.id.clone()) {
                return Err(CoreError::Validation(format!(
                    "duplicate step id `{}` in plan",
                    step.id
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CoreConfig;
    use crate::llm::NimClient;
    use std::path::Path;

    fn sample_profile() -> SystemProfile {
        SystemProfile {
            os_name: "windows".to_string(),
            os_version: "Windows 11".to_string(),
            arch: "x86_64".to_string(),
            gpu_detected: Some("NVIDIA RTX 4080".to_string()),
            free_disk_bytes: 100_000_000_000,
            installed_tools: vec![],
            active_path_entries: vec![],
        }
    }

    #[tokio::test]
    async fn planner_matches_deterministic_cpp20_recipe_first() {
        let store = RecipeStore::load_from_dir_or_embedded(Path::new("nonexistent"))
            .expect("embedded recipes should load");
        let mut cfg = CoreConfig::from_env();
        cfg.nim_mock = true;
        let nim = Arc::new(NimClient::new(cfg).expect("nim client"));
        let planner = Planner::new(store, nim, EventBus::default());

        let plan = planner
            .create_plan("set me up for C++20 dev", &sample_profile(), None)
            .await
            .expect("plan should succeed");

        assert_eq!(
            plan.source,
            crate::types::PlanSource::Recipe {
                recipe_id: "cpp20_dev".to_string()
            }
        );
        assert_eq!(plan.steps.len(), 5);
    }

    #[tokio::test]
    async fn planner_falls_back_to_nim_mock_for_unmatched_goal() {
        let store = RecipeStore::load_from_dir_or_embedded(Path::new("nonexistent"))
            .expect("embedded recipes should load");
        let mut cfg = CoreConfig::from_env();
        cfg.nim_mock = true;
        let nim = Arc::new(NimClient::new(cfg).expect("nim client"));
        let planner = Planner::new(store, nim, EventBus::default());

        let plan = planner
            .create_plan("set up a Node.js and TypeScript backend", &sample_profile(), None)
            .await
            .expect("fallback plan should succeed");

        assert!(matches!(plan.source, crate::types::PlanSource::Llm { .. }));
        assert_eq!(plan.steps.len(), 3);
    }
}
