use std::env;
use std::path::PathBuf;
use thiserror::Error;

/// Top-level error enum for `installer_core`.
/// Zero `.unwrap()` or `.expect()` calls are permitted in production code.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("OS provider error: {0}")]
    Os(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP / network error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("LLM planning failed: {0}")]
    Llm(String),

    #[error("plan validation error: {0}")]
    Validation(String),

    #[error("adapter error: {0}")]
    Adapter(#[from] crate::adapters::AdapterError),
}

/// Runtime configuration loaded from environment variables (with safe defaults).
#[derive(Debug, Clone)]
pub struct CoreConfig {
    pub nim_base_url: String,
    pub nim_model: String,
    pub nim_api_key: Option<String>,
    pub nim_mock: bool,
    pub tavily_api_key: Option<String>,
    pub tavily_mock: bool,
    pub recipes_dir: PathBuf,
    pub state_log_path: PathBuf,
}

impl CoreConfig {
    pub const DEFAULT_NIM_BASE_URL: &'static str = "https://integrate.api.nvidia.com/v1";
    pub const DEFAULT_NIM_MODEL: &'static str = "nvidia/llama-3.1-nemotron-70b-instruct";

    /// Loads configuration from process environment variables.
    /// Automatically loads a `.env` file if present in the current directory.
    pub fn from_env() -> Self {
        Self::load_dotenv_file(PathBuf::from(".env"));

        let nim_base_url = env::var("NIM_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| Self::DEFAULT_NIM_BASE_URL.to_string());

        let nim_model = env::var("NIM_MODEL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| Self::DEFAULT_NIM_MODEL.to_string());

        let nim_api_key = env::var("NIM_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());

        let nim_mock = match env::var("NIM_MOCK") {
            Ok(val) => matches!(
                val.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            Err(_) => nim_api_key.is_none(),
        };

        let tavily_api_key = env::var("TAVILY_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());

        let tavily_mock = match env::var("TAVILY_MOCK") {
            Ok(val) => matches!(
                val.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            Err(_) => tavily_api_key.is_none(),
        };

        let recipes_dir = env::var("ORCHESTRATOR_RECIPES_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("recipes"));

        let state_log_path = env::var("ORCHESTRATOR_STATE_LOG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".orchestrator_state.jsonl"));

        Self {
            nim_base_url,
            nim_model,
            nim_api_key,
            nim_mock,
            tavily_api_key,
            tavily_mock,
            recipes_dir,
            state_log_path,
        }
    }

    /// Minimal, dependency-free `.env` parser so `.env` works out of the box.
    fn load_dotenv_file(path: PathBuf) {
        let Ok(contents) = std::fs::read_to_string(path) else {
            return;
        };
        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let k = key.trim();
                let v = value.trim().trim_matches('"').trim_matches('\'');
                if !k.is_empty() && env::var_os(k).is_none() {
                    env::set_var(k, v);
                }
            }
        }
    }
}
