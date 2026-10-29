pub mod sanitizer;

use async_trait::async_trait;
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{CoreConfig, CoreError};
use crate::types::{Plan, SystemProfile};

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

/// OpenAI-compatible inference client for NVIDIA NIM or Nebius
/// speaking `/v1/chat/completions`
/// with `schemars` JSON Schema enforcement and an offline deterministic mock mode (`NIM_MOCK=1`).
#[derive(Debug, Clone)]
pub struct NimClient {
    config: CoreConfig,
    http: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    response_format: ResponseFormat,
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

impl NimClient {
    pub fn new(config: CoreConfig) -> Result<Self, CoreError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(45))
            .build()?;
        Ok(Self { config, http })
    }

    /// Generates the canonical JSON Schema for `Plan` using `schemars`.
    pub fn plan_json_schema() -> Result<Value, CoreError> {
        let root_schema = schema_for!(Plan);
        Ok(serde_json::to_value(&root_schema)?)
    }

    async fn provider_settings(&self) -> Result<(String, String, String), CoreError> {
        if let Some(api_key) = self
            .config
            .nebius_api_key
            .as_deref()
            .filter(|key| !key.trim().is_empty())
        {
            let model = if let Some(model) = self.config.nebius_model.as_deref() {
                model.to_string()
            } else {
                let response = self
                    .http
                    .get(format!(
                        "{}/models",
                        self.config.nebius_base_url.trim_end_matches('/')
                    ))
                    .bearer_auth(api_key)
                    .send()
                    .await
                    .map_err(|error| {
                        CoreError::Llm(format!(
                            "Could not retrieve Nebius models: {}",
                            error.without_url()
                        ))
                    })?;
                if !response.status().is_success() {
                    return Err(CoreError::Llm(format!(
                        "Nebius model lookup returned HTTP {}.",
                        response.status()
                    )));
                }
                let payload: Value = response.json().await?;
                payload
                    .get("data")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item.get("id").and_then(Value::as_str))
                    .find(|model| model.to_ascii_lowercase().contains("nemotron"))
                    .ok_or_else(|| {
                        CoreError::Llm(
                            "No Nemotron model is available for this Nebius account.".to_string(),
                        )
                    })?
                    .to_string()
            };
            return Ok((
                api_key.to_string(),
                self.config.nebius_base_url.clone(),
                model,
            ));
        }

        let api_key = self
            .config
            .nim_api_key
            .as_deref()
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| {
                CoreError::Config(
                    "Set NEBIUS_API_KEY or NIM_API_KEY in your local .env file.".to_string(),
                )
            })?;
        Ok((
            api_key.to_string(),
            self.config.nim_base_url.clone(),
            self.config.nim_model.clone(),
        ))
    }
}

#[async_trait]
impl LlmClient for NimClient {
    fn model_name(&self) -> &str {
        if self.config.nebius_api_key.is_some() {
            self.config
                .nebius_model
                .as_deref()
                .unwrap_or("Nemotron selected from live model list")
        } else {
            &self.config.nim_model
        }
    }

    async fn propose_plan(
        &self,
        goal: &str,
        profile: &SystemProfile,
        untrusted_context: Option<&str>,
    ) -> Result<Plan, CoreError> {
        let (api_key, base_url, model) = self.provider_settings().await?;

        let schema_value = Self::plan_json_schema()?;
        let system_prompt = sanitizer::build_hardened_system_prompt(&schema_value)?;
        let user_prompt = sanitizer::build_user_prompt(goal, profile, untrusted_context)?;

        let endpoint = format!(
            "{}/chat/completions",
            base_url.trim_end_matches('/')
        );

        let request_body = ChatCompletionRequest {
            model,
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
            max_tokens: Some(1500),
            response_format: ResponseFormat {
                kind: "json_schema".to_string(),
                json_schema: JsonSchemaPayload {
                    name: "zero_trust_execution_plan".to_string(),
                    strict: true,
                    schema: schema_value,
                },
            },
        };

        let response = self
            .http
            .post(&endpoint)
            .bearer_auth(&api_key)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(CoreError::Llm(format!(
                "AI provider returned HTTP {status}."
            )));
        }

        let completion: ChatCompletionResponse = response.json().await?;
        let raw_content = completion
            .choices
            .first()
            .map(|c| c.message.content.trim())
            .ok_or_else(|| CoreError::Llm("NVIDIA NIM returned empty choices".to_string()))?;

        let cleaned = sanitizer::strip_markdown_code_fences(raw_content);
        let plan: Plan = serde_json::from_str(cleaned).map_err(|e| {
            CoreError::Llm(format!(
                "LLM response did not conform to Plan JSON Schema: {e}. Raw: {cleaned}"
            ))
        })?;

        Ok(plan)
    }

    async fn propose_remediation_plan(
        &self,
        goal: &str,
        profile: &SystemProfile,
        failed_step_id: &str,
        diagnostic: &str,
        attempt: u8,
    ) -> Result<Plan, CoreError> {
        let (api_key, base_url, model) = self.provider_settings().await?;

        let schema_value = Self::plan_json_schema()?;
        let system_prompt = sanitizer::build_hardened_system_prompt(&schema_value)?;
        let user_prompt = sanitizer::build_remediation_user_prompt(
            goal,
            profile,
            failed_step_id,
            diagnostic,
            attempt,
        )?;

        let endpoint = format!(
            "{}/chat/completions",
            base_url.trim_end_matches('/')
        );

        let request_body = ChatCompletionRequest {
            model,
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
            max_tokens: Some(1500),
            response_format: ResponseFormat {
                kind: "json_schema".to_string(),
                json_schema: JsonSchemaPayload {
                    name: "zero_trust_execution_plan".to_string(),
                    strict: true,
                    schema: schema_value,
                },
            },
        };

        let response = self
            .http
            .post(&endpoint)
            .bearer_auth(&api_key)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(CoreError::Llm(format!(
                "AI provider returned HTTP {status}."
            )));
        }

        let completion: ChatCompletionResponse = response.json().await?;
        let raw_content = completion
            .choices
            .first()
            .map(|c| c.message.content.trim())
            .ok_or_else(|| CoreError::Llm("NVIDIA NIM returned empty choices".to_string()))?;

        let cleaned = sanitizer::strip_markdown_code_fences(raw_content);
        let mut plan: Plan = serde_json::from_str(cleaned).map_err(|e| {
            CoreError::Llm(format!(
                "LLM response did not conform to Plan JSON Schema: {e}. Raw: {cleaned}"
            ))
        })?;

        plan.source = crate::types::PlanSource::Healer {
            failed_step_id: failed_step_id.to_string(),
            attempt,
        };

        Ok(plan)
    }
}
