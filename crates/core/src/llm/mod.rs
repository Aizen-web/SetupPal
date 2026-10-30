#[cfg(test)]
pub mod mock;
pub mod models;
pub mod provider;
pub mod sanitizer;

use std::sync::Arc;

use async_trait::async_trait;
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{CoreConfig, CoreError};
use crate::types::{Plan, SystemProfile};

pub use crate::config::NemotronModel;
pub use models::{ModelSelection, DEFAULT_MODEL_SLUG};
pub use provider::{
    resolve_provider, resolve_with_selection, select_provider, selected_provider, ConfigKeyStore,
    KeyStore, ProviderKind, ResolvedProvider,
};

#[async_trait]
pub trait LlmClient: Send + Sync {
    fn model_name(&self) -> &str;

    async fn propose_plan(
        &self,
        goal: &str,
        profile: &SystemProfile,
        untrusted_context: Option<&str>,
    ) -> Result<Plan, CoreError>;

    async fn propose_remediation_plan(
        &self,
        goal: &str,
        profile: &SystemProfile,
        failed_step_id: &str,
        diagnostic: &str,
        attempt: u8,
    ) -> Result<Plan, CoreError>;
}

/// OpenAI-compatible `/v1/chat/completions` client for Nebius Token Factory, with `schemars`
/// JSON Schema enforcement for plans. Model selection is independent of the Safety Engine.
#[derive(Clone)]
pub struct ProviderClient {
    config: CoreConfig,
    keys: Arc<dyn KeyStore>,
    models: ModelSelection,
    http: reqwest::Client,
}

/// Former name, still re-exported by `lib.rs` and used by the CLI.
pub type NimClient = ProviderClient;

impl std::fmt::Debug for ProviderClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderClient")
            .field("config", &self.config)
            .field("model", &self.models.get().slug())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    response_format: ResponseFormat,
    chat_template_kwargs: ChatTemplateKwargs,
}

#[derive(Debug, Serialize)]
struct ChatTemplateKwargs {
    enable_thinking: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: String,
    json_schema: JsonSchemaPayload,
}

#[derive(Debug, Serialize)]
struct JsonSchemaPayload {
    name: String,
    strict: bool,
    schema: Value,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

impl ProviderClient {
    /// Uses the keys captured in `config` at startup and an ephemeral model selection.
    pub fn new(config: CoreConfig) -> Result<Self, CoreError> {
        let model = config.model;
        let keys = Arc::new(ConfigKeyStore::from_config(&config));
        Self::with_key_store_and_models(config, keys, ModelSelection::ephemeral(model))
    }

    pub fn with_key_store(config: CoreConfig, keys: Arc<dyn KeyStore>) -> Result<Self, CoreError> {
        let models = ModelSelection::ephemeral(config.model);
        Self::with_key_store_and_models(config, keys, models)
    }

    pub fn with_key_store_and_models(
        config: CoreConfig,
        keys: Arc<dyn KeyStore>,
        models: ModelSelection,
    ) -> Result<Self, CoreError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;
        Ok(Self {
            config,
            keys,
            models,
            http,
        })
    }

    /// Generates the canonical JSON Schema for `Plan` using `schemars`.
    pub fn plan_json_schema() -> Result<Value, CoreError> {
        let root_schema = schema_for!(Plan);
        Ok(serde_json::to_value(&root_schema)?)
    }

    pub fn config(&self) -> &CoreConfig {
        &self.config
    }

    pub fn key_store(&self) -> &dyn KeyStore {
        self.keys.as_ref()
    }

    pub fn model_selection(&self) -> &ModelSelection {
        &self.models
    }

    pub fn selected_model(&self) -> NemotronModel {
        self.models.get()
    }

    /// The provider, model, and key every AI call uses right now.
    pub fn resolve(&self) -> Result<ResolvedProvider, CoreError> {
        resolve_with_selection(&self.config, self.keys.as_ref(), &self.models)
    }

    /// The selected provider even when its key is missing (for status display).
    pub fn selected(&self) -> Result<ProviderKind, CoreError> {
        selected_provider(&self.config)
    }

    async fn request_plan(&self, user_prompt: String) -> Result<Plan, CoreError> {
        let provider = self.resolve()?;
        let schema_value = Self::plan_json_schema()?;
        let system_prompt = sanitizer::build_hardened_system_prompt(&schema_value)?;
        let model_id = provider.model_id();

        let request_body = ChatCompletionRequest {
            model: model_id.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_prompt,
                },
            ],
            temperature: 0.0,
            max_tokens: Some(4096),
            response_format: ResponseFormat {
                kind: "json_schema".to_string(),
                json_schema: JsonSchemaPayload {
                    name: "zero_trust_execution_plan".to_string(),
                    strict: true,
                    schema: schema_value,
                },
            },
            chat_template_kwargs: ChatTemplateKwargs {
                enable_thinking: false,
            },
        };

        let response = self
            .http
            .post(provider.chat_completions_url())
            .bearer_auth(provider.api_key())
            .json(&request_body)
            .send()
            .await
            .map_err(|error| {
                CoreError::Llm(format!(
                    "Could not reach {}: {}",
                    provider.kind.display_name(),
                    error.without_url()
                ))
            })?;

        if !response.status().is_success() {
            return Err(CoreError::Llm(format!(
                "{} returned HTTP {} for model `{}` ({}). No other model was tried.",
                provider.kind.display_name(),
                response.status(),
                provider.model.display_name(),
                model_id
            )));
        }

        let completion: ChatCompletionResponse = response.json().await.map_err(|error| {
            CoreError::Llm(format!(
                "{} sent an unreadable response: {}",
                provider.kind.display_name(),
                error.without_url()
            ))
        })?;
        let raw_content = completion
            .choices
            .first()
            .map(|c| c.message.content.trim())
            .ok_or_else(|| {
                CoreError::Llm(format!(
                    "{} returned no choices for model `{}`.",
                    provider.kind.display_name(),
                    model_id
                ))
            })?;

        let cleaned = sanitizer::strip_markdown_code_fences(raw_content);
        serde_json::from_str(cleaned).map_err(|e| {
            CoreError::Llm(format!(
                "LLM response did not conform to Plan JSON Schema: {e}. Raw: {cleaned}"
            ))
        })
    }
}

#[async_trait]
impl LlmClient for ProviderClient {
    fn model_name(&self) -> &str {
        self.selected_model().api_id()
    }

    async fn propose_plan(
        &self,
        goal: &str,
        profile: &SystemProfile,
        untrusted_context: Option<&str>,
    ) -> Result<Plan, CoreError> {
        let user_prompt = sanitizer::build_user_prompt(goal, profile, untrusted_context)?;
        self.request_plan(user_prompt).await
    }

    async fn propose_remediation_plan(
        &self,
        goal: &str,
        profile: &SystemProfile,
        failed_step_id: &str,
        diagnostic: &str,
        attempt: u8,
    ) -> Result<Plan, CoreError> {
        let user_prompt = sanitizer::build_remediation_user_prompt(
            goal,
            profile,
            failed_step_id,
            diagnostic,
            attempt,
        )?;
        let mut plan = self.request_plan(user_prompt).await?;
        plan.source = crate::types::PlanSource::Healer {
            failed_step_id: failed_step_id.to_string(),
            attempt,
        };
        Ok(plan)
    }
}
