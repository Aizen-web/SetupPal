use std::sync::Arc;

use crate::config::CoreError;
use crate::events::{AgentEvent, EventBus};
use crate::llm::LlmClient;
use crate::os::OsProvider;
use crate::plan_scope::validate_plan_scope;
use crate::recipes::RecipeStore;
use crate::resolver::{
    close_popular_match, is_exact_popular_name, parse_registry_goal, resolve_winget_id,
    single_registry_plan, single_winget_plan,
};
use strsim::levenshtein;
use crate::types::{Plan, SystemProfile};

/// Converts `Goal + SystemProfile + Optional<UntrustedContext>` into a validated `Plan`.
/// Checks deterministic `RecipeStore` first, then the winget resolver, then `LlmClient`.
pub struct Planner {
    recipes: RecipeStore,
    llm: Arc<dyn LlmClient>,
    events: EventBus,
    os: Arc<dyn OsProvider>,
}

impl Planner {
    pub fn new(
        recipes: RecipeStore,
        llm: Arc<dyn LlmClient>,
        events: EventBus,
        os: Arc<dyn OsProvider>,
    ) -> Self {
        Self {
            recipes,
            llm,
            events,
            os,
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

        // 1. Stack-phrase recipes only (single-app words never match — see Recipe::matches_goal)
        if untrusted_context.is_none() {
            if let Some(recipe) = self.recipes.find_matching(trimmed_goal) {
                self.events.emit(AgentEvent::RecipeMatched {
                    recipe_id: recipe.id.clone(),
                });
                let plan = recipe.to_plan(trimmed_goal);
                self.validate_plan_structure(&plan)?;
                validate_plan_scope(&plan)?;
                self.events.emit(AgentEvent::PlanGenerated {
                    plan: Box::new(plan.clone()),
                });
                return Ok(plan);
            }
        }

        // 2. pip/npm: exact popular names plan directly; close typos ask first (R11).
        if untrusted_context.is_none() {
            if let Some((eco, name)) = parse_registry_goal(trimmed_goal) {
                if is_exact_popular_name(eco, &name) {
                    let plan = single_registry_plan(eco, &name, None);
                    self.validate_plan_structure(&plan)?;
                    validate_plan_scope(&plan)?;
                    self.events.emit(AgentEvent::PlanGenerated {
                        plan: Box::new(plan.clone()),
                    });
                    return Ok(plan);
                }
                if let Some(suggestion) = close_popular_match(eco, &name) {
                    return Err(CoreError::Validation(format!(
                        "confirm:{eco:?}:{name}:{suggestion}"
                    )));
                }
            }
        }

        // 3. Deterministic winget resolve (exact ID or one clear search hit)
        if untrusted_context.is_none() {
            if let Some(id) = resolve_winget_id(self.os.as_ref(), trimmed_goal).await {
                let plan = single_winget_plan(trimmed_goal, &id);
                self.validate_plan_structure(&plan)?;
                validate_plan_scope(&plan)?;
                self.events.emit(AgentEvent::PlanGenerated {
                    plan: Box::new(plan.clone()),
                });
                return Ok(plan);
            }
        }

        // 4. Fall back to the pinned provider with JSON Schema enforcement
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

        // Reject LLM plans that silently rename a typed registry package (R11).
        if let Some((eco, typed)) = parse_registry_goal(trimmed_goal) {
            for step in &plan.steps {
                if let crate::types::Step::InstallPackage {
                    ecosystem,
                    package_id,
                    ..
                } = &step.step
                {
                    if *ecosystem == eco
                        && !package_id.eq_ignore_ascii_case(&typed)
                        && (1..=2).contains(&levenshtein(
                            &typed.to_ascii_lowercase(),
                            &package_id.to_ascii_lowercase(),
                        ))
                    {
                        return Err(CoreError::Validation(format!(
                            "confirm:{eco:?}:{typed}:{package_id}"
                        )));
                    }
                }
            }
        }

        self.validate_plan_structure(&plan)?;
        validate_plan_scope(&plan)?;

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
    use crate::os::mock::{ok, MockOsProvider};
    use crate::types::{AllowedBinary, Ecosystem, PlanSource, Step};

    fn planner_with(llm: Arc<MockLlmClient>) -> (Planner, EventBus) {
        let events = EventBus::default();
        let recipes = RecipeStore::load_from_dir_or_embedded(Path::new("does-not-exist"))
            .expect("embedded recipes load");
        let os = Arc::new(MockOsProvider::new());
        (
            Planner::new(recipes, llm, events.clone(), os),
            events,
        )
    }

    fn planner_with_os(llm: Arc<MockLlmClient>, os: Arc<MockOsProvider>) -> (Planner, EventBus) {
        let events = EventBus::default();
        let recipes = RecipeStore::load_from_dir_or_embedded(Path::new("does-not-exist"))
            .expect("embedded recipes load");
        (
            Planner::new(recipes, llm, events.clone(), os),
            events,
        )
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
            .create_plan("set me up for rust", &sample_profile(), None)
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
    async fn pip_typos_ask_instead_of_planning_a_rename() {
        let llm = Arc::new(MockLlmClient::new());
        let (planner, _) = planner_with(llm.clone());
        for goal in ["pip install reqeusts", "npm i lodahs", "pip install colourama"] {
            let err = planner
                .create_plan(goal, &sample_profile(), None)
                .await
                .expect_err(goal);
            let msg = err.to_string();
            assert!(msg.contains("confirm:"), "{goal} -> {msg}");
            assert!(!msg.contains("InstallPackage"), "{goal}");
        }
        assert!(llm.calls().is_empty());
    }

    #[tokio::test]
    async fn exact_pip_package_plans_without_the_model() {
        let llm = Arc::new(MockLlmClient::new());
        let (planner, _) = planner_with(llm.clone());
        let plan = planner
            .create_plan("pip install requests", &sample_profile(), None)
            .await
            .expect("exact");
        assert!(matches!(
            &plan.steps[0].step,
            Step::InstallPackage { package_id, ecosystem: Ecosystem::PyPI, .. }
                if package_id == "requests"
        ));
        assert!(llm.calls().is_empty());
    }

    #[tokio::test]
    async fn single_app_resolves_without_the_model() {
        let llm = Arc::new(MockLlmClient::new());
        let os = Arc::new(
            MockOsProvider::new()
                .with_binary("winget", r"C:\winget.exe")
                .on(
                    "winget",
                    &["search"],
                    ok("Name              Id            Version Source\n---------------------------------------------\nVLC media player  VideoLAN.VLC  3.0.21  winget\n"),
                )
                .on(
                    "winget",
                    &["show"],
                    ok("Found VLC [VideoLAN.VLC]\nInstaller Type: nullsoft\n"),
                ),
        );
        let (planner, _) = planner_with_os(llm.clone(), os);
        let plan = planner
            .create_plan("install vlc please", &sample_profile(), None)
            .await
            .expect("resolved");
        assert!(matches!(plan.source, PlanSource::Resolved { .. }));
        assert_eq!(plan.steps.len(), 1);
        assert!(matches!(
            &plan.steps[0].step,
            Step::InstallPackage { package_id, .. } if package_id == "VideoLAN.VLC"
        ));
        assert!(llm.calls().is_empty());
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
            .create_plan("set me up for rust", &sample_profile(), Some("README text"))
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
            .create_plan_with_context("something obscure xyzzy", &sample_profile(), None, Some("User wants CLI tools"))
            .await
            .expect("plan");
        assert_eq!(plan.goal, "something obscure xyzzy");
        match &llm.calls()[0] {
            RecordedLlmCall::Plan { goal, .. } => {
                assert!(goal.starts_with("something obscure xyzzy"));
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
            .create_plan("something obscure xyzzy", &sample_profile(), None)
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
        let first = planner
            .create_plan("something obscure xyzzy", &sample_profile(), None)
            .await;
        assert!(first
            .expect_err("duplicate")
            .to_string()
            .contains("duplicate step id"));
        let second = planner
            .create_plan("something obscure xyzzy", &sample_profile(), None)
            .await;
        assert!(second.expect_err("blank").to_string().contains("empty id"));
    }

    #[tokio::test]
    async fn model_errors_are_returned_not_swallowed() {
        let llm = Arc::new(MockLlmClient::new().with_plan_error("provider down"));
        let (planner, _) = planner_with(llm);
        let err = planner
            .create_plan("something obscure xyzzy", &sample_profile(), None)
            .await
            .expect_err("llm error");
        assert!(err.to_string().contains("provider down"));
    }

    #[tokio::test]
    async fn llm_plan_with_unrequested_package_is_rejected() {
        use crate::types::{AdapterKind, Ecosystem, PlannedStep};
        let bad = Plan {
            plan_id: "p".into(),
            goal: "install vlc".into(),
            summary: "x".into(),
            source: PlanSource::Llm {
                model: "test".into(),
            },
            steps: vec![
                PlannedStep {
                    id: "step-1".into(),
                    description: "vlc".into(),
                    step: Step::InstallPackage {
                        ecosystem: Ecosystem::Winget,
                        package_id: "VideoLAN.VLC".into(),
                        version: None,
                        source_adapter: AdapterKind::Winget,
                    },
                },
                PlannedStep {
                    id: "step-2".into(),
                    description: "git".into(),
                    step: Step::InstallPackage {
                        ecosystem: Ecosystem::Winget,
                        package_id: "Git.Git".into(),
                        version: None,
                        source_adapter: AdapterKind::Winget,
                    },
                },
            ],
        };
        let llm = Arc::new(MockLlmClient::new().with_plan(bad));
        let (planner, _) = planner_with(llm);
        let err = planner
            .create_plan("install vlc", &sample_profile(), None)
            .await
            .expect_err("scope");
        assert!(err.to_string().contains("unrequested"));
    }
}
