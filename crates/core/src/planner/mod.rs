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
        self.create_plan_with_context(goal, profile, untrusted_context, None)
            .await
    }

    /// Like `create_plan`, but `conversation_context` is appended to the LLM prompt only
    /// (never treated as untrusted README context, so recipe matching still runs).
    pub async fn create_plan_with_context(
        &self,
        goal: &str,
        profile: &SystemProfile,
        untrusted_context: Option<&str>,
        conversation_context: Option<&str>,
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

        // 2. Fall back to the pinned provider with JSON Schema enforcement
        self.events.emit(AgentEvent::LlmPlanningFallback {
            goal: trimmed_goal.to_string(),
            model: self.llm.model_name().to_string(),
        });

        let llm_goal = match conversation_context
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            Some(ctx) => {
                format!("{trimmed_goal}\n\nConversation summary (for planning only):\n{ctx}")
            }
            None => trimmed_goal.to_string(),
        };

        let mut plan = self
            .llm
            .propose_plan(&llm_goal, profile, untrusted_context)
            .await?;
        plan.goal = trimmed_goal.to_string();

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
    use std::path::Path;

    use super::*;
    use crate::llm::mock::{llm_plan, sample_profile, verify_step, MockLlmClient, RecordedLlmCall};
    use crate::types::{AllowedBinary, PlanSource};

    fn planner_with(llm: Arc<MockLlmClient>) -> (Planner, EventBus) {
        let events = EventBus::default();
        let recipes = RecipeStore::load_from_dir_or_embedded(Path::new("does-not-exist"))
            .expect("embedded recipes load");
        (Planner::new(recipes, llm, events.clone()), events)
    }

    fn one_step_plan(goal: &str) -> Plan {
        llm_plan(goal, vec![verify_step("step-1", AllowedBinary::Git)])
    }

    #[tokio::test]
    async fn empty_goal_is_rejected_before_any_model_call() {
        let llm = Arc::new(MockLlmClient::new());
        let (planner, _) = planner_with(llm.clone());
        let err = planner
            .create_plan("   ", &sample_profile(), None)
            .await
            .expect_err("empty goal");
        assert!(matches!(err, CoreError::Validation(_)));
        assert!(llm.calls().is_empty());
    }

    #[tokio::test]
    async fn recipe_match_never_calls_the_model() {
        let llm = Arc::new(MockLlmClient::new());
        let (planner, events) = planner_with(llm.clone());
        let mut rx = events.subscribe();
        let plan = planner
            .create_plan("set me up for Rust", &sample_profile(), None)
            .await
            .expect("recipe plan");
        assert!(matches!(plan.source, PlanSource::Recipe { .. }));
        assert!(!plan.steps.is_empty());
        assert!(llm.calls().is_empty());
        let mut saw_recipe = false;
        while let Ok(event) = rx.try_recv() {
            saw_recipe |= matches!(event, AgentEvent::RecipeMatched { .. });
        }
        assert!(saw_recipe);
    }

    #[tokio::test]
    async fn unmatched_goal_uses_the_model_and_keeps_the_users_goal() {
        let llm = Arc::new(MockLlmClient::new().with_plan(one_step_plan("model-invented goal")));
        let (planner, _) = planner_with(llm.clone());
        let plan = planner
            .create_plan("  jq and ripgrep please  ", &sample_profile(), None)
            .await
            .expect("llm plan");
        assert_eq!(plan.goal, "jq and ripgrep please");
        assert_eq!(
            llm.calls(),
            vec![RecordedLlmCall::Plan {
                goal: "jq and ripgrep please".to_string(),
                untrusted_context: None,
            }]
        );
    }

    #[tokio::test]
    async fn untrusted_context_skips_recipes_and_is_passed_through() {
        let llm = Arc::new(MockLlmClient::new().with_plan(one_step_plan("x")));
        let (planner, _) = planner_with(llm.clone());
        planner
            .create_plan("set me up for Rust", &sample_profile(), Some("README text"))
            .await
            .expect("llm plan");
        match &llm.calls()[0] {
            RecordedLlmCall::Plan {
                untrusted_context, ..
            } => {
                assert_eq!(untrusted_context.as_deref(), Some("README text"));
            }
            other => panic!("unexpected call {other:?}"),
        }
    }

    #[tokio::test]
    async fn conversation_context_is_appended_for_the_model_only() {
        let llm = Arc::new(MockLlmClient::new().with_plan(one_step_plan("x")));
        let (planner, _) = planner_with(llm.clone());
        let plan = planner
            .create_plan_with_context("jq", &sample_profile(), None, Some("User wants CLI tools"))
            .await
            .expect("plan");
        assert_eq!(plan.goal, "jq");
        match &llm.calls()[0] {
            RecordedLlmCall::Plan { goal, .. } => {
                assert!(goal.starts_with("jq"));
                assert!(goal.contains("User wants CLI tools"));
            }
            other => panic!("unexpected call {other:?}"),
        }
    }

    #[tokio::test]
    async fn model_plan_with_no_steps_is_rejected() {
        let llm = Arc::new(MockLlmClient::new().with_plan(llm_plan("g", vec![])));
        let (planner, _) = planner_with(llm);
        let err = planner
            .create_plan("jq", &sample_profile(), None)
            .await
            .expect_err("empty plan");
        assert!(err.to_string().contains("zero steps"));
    }

    #[tokio::test]
    async fn duplicate_and_blank_step_ids_are_rejected() {
        let dup = llm_plan(
            "g",
            vec![
                verify_step("step-1", AllowedBinary::Git),
                verify_step("step-1", AllowedBinary::Node),
            ],
        );
        let blank = llm_plan("g", vec![verify_step("  ", AllowedBinary::Git)]);
        let llm = Arc::new(MockLlmClient::new().with_plan(dup).with_plan(blank));
        let (planner, _) = planner_with(llm);
        let first = planner.create_plan("jq", &sample_profile(), None).await;
        assert!(first
            .expect_err("duplicate")
            .to_string()
            .contains("duplicate step id"));
        let second = planner.create_plan("jq", &sample_profile(), None).await;
        assert!(second.expect_err("blank").to_string().contains("empty id"));
    }

    #[tokio::test]
    async fn model_errors_are_returned_not_swallowed() {
        let llm = Arc::new(MockLlmClient::new().with_plan_error("provider down"));
        let (planner, _) = planner_with(llm);
        let err = planner
            .create_plan("jq", &sample_profile(), None)
            .await
            .expect_err("llm error");
        assert!(err.to_string().contains("provider down"));
    }
}
