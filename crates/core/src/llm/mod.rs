pub mod mock;
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
}

/// NVIDIA NIM client speaking OpenAI-compatible `/v1/chat/completions`
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
}

#[async_trait]
impl LlmClient for NimClient {
    fn model_name(&self) -> &str {
        if self.config.nim_mock {
            "nim-mock-deterministic"
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
        if self.config.nim_mock {
            return mock::generate_mock_plan(goal, profile, untrusted_context);
        }

        let api_key = self.config.nim_api_key.as_deref().ok_or_else(|| {
            CoreError::Config(
                "NIM_API_KEY is not set and NIM_MOCK is disabled. Set NIM_API_KEY or NIM_MOCK=1."
                    .to_string(),
            )
        })?;

        let schema_value = Self::plan_json_schema()?;
        let system_prompt = sanitizer::build_hardened_system_prompt(&schema_value)?;
        let user_prompt = sanitizer::build_user_prompt(goal, profile, untrusted_context)?;

        let endpoint = format!(
            "{}/chat/completions",
            self.config.nim_base_url.trim_end_matches('/')
        );

        let request_body = ChatCompletionRequest {
            model: self.config.nim_model.clone(),
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
            .bearer_auth(api_key)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            return Err(CoreError::Llm(format!(
                "NVIDIA NIM HTTP {status}: {err_text}"
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
}
