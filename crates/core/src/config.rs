use std::env;
use std::path::PathBuf;
use thiserror::Error;

use serde::{Deserialize, Serialize};

/// Looks up a secret (for example a Tavily or GitHub key) at the moment it is needed, so a key
/// saved while the app is running takes effect without a restart. Never log what it returns.
pub type SecretProvider = std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// The only AI models SetupPal may call. IDs match the Nebius Token Factory catalog.
/// Model choice never touches the Safety Engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NemotronModel {
    /// Fast everyday planning (`nvidia/Nemotron-3_5-Lightning`).
    Lightning,
    /// Default: balanced reasoning (`nvidia/nemotron-3-super-120b-a12b`).
    Super,
    /// Advanced multi-stage setups (`nvidia/Nemotron-3-Ultra-550b-a55b`).
    Ultra,
}

impl Default for NemotronModel {
    fn default() -> Self {
        Self::Super
    }
}

impl NemotronModel {
    pub const ALL: [Self; 3] = [Self::Lightning, Self::Super, Self::Ultra];

    /// Stable Settings / API slug (`lightning` | `super` | `ultra`).
    pub fn slug(self) -> &'static str {
        match self {
            Self::Lightning => "lightning",
            Self::Super => "super",
            Self::Ultra => "ultra",
        }
    }

    /// Exact Nebius Token Factory model ID. Never invent IDs — only catalog values.
    pub fn api_id(self) -> &'static str {
        match self {
            Self::Lightning => "nvidia/Nemotron-3_5-Lightning",
            Self::Super => "nvidia/nemotron-3-super-120b-a12b",
            Self::Ultra => "nvidia/Nemotron-3-Ultra-550b-a55b",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Lightning => "Nemotron 3.5 Lightning",
            Self::Super => "Nemotron 3 Super",
            Self::Ultra => "Nemotron 3 Ultra",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Lightning => "Fast, everyday installation planning",
            Self::Super => "Recommended default. Balanced reasoning and planning",
            Self::Ultra => "Advanced reasoning for complicated, multi-stage setups",
        }
    }

    /// Accepts a Settings slug or a full Nebius API ID. Rejects anything else.
    pub fn parse(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        let lower = trimmed.to_ascii_lowercase();
        Self::ALL.into_iter().find(|model| {
            model.slug() == lower
                || model.api_id().eq_ignore_ascii_case(trimmed)
                || match model {
                    Self::Lightning => {
                        lower == "nemotron-3.5-lightning" || lower == "nemotron_3_5-lightning"
                    }
                    Self::Super => {
                        lower == "nemotron-3-super" || lower == "nemotron-3-super-120b-a12b"
                    }
                    Self::Ultra => {
                        lower == "nemotron-3-ultra" || lower == "nemotron-3-ultra-550b-a55b"
                    }
                }
        })
    }

    pub fn parse_or_err(value: &str) -> Result<Self, CoreError> {
        Self::parse(value).ok_or_else(|| {
            CoreError::Config(format!(
                "Unknown model `{value}`. Choose one of: {}.",
                Self::ALL
                    .iter()
                    .map(|m| format!("{} (`{}`)", m.slug(), m.display_name()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })
    }

    /// Public catalog entry for `/api/health` and Settings.
    pub fn catalog_entry(self) -> serde_json::Value {
        serde_json::json!({
            "id": self.slug(),
            "api_id": self.api_id(),
            "name": self.display_name(),
            "description": self.description(),
            "default": self == Self::default(),
        })
    }

    pub fn catalog() -> Vec<serde_json::Value> {
        Self::ALL.iter().map(|m| m.catalog_entry()).collect()
    }
}

/// A provider that always returns the same value (or nothing).
pub fn fixed_secret(value: Option<String>) -> SecretProvider {
    std::sync::Arc::new(move || value.clone())
}

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
#[derive(Clone)]
pub struct CoreConfig {
    /// Raw `SETUPPAL_PROVIDER` value; must be `nebius` or unset. Validated by `llm::provider::select_provider`.
    pub provider: Option<String>,
    pub nebius_base_url: String,
    pub nebius_api_key: Option<String>,
    pub tavily_api_key: Option<String>,
    pub recipes_dir: PathBuf,
    pub state_log_path: PathBuf,
    /// Initial Nemotron model from `SETUPPAL_MODEL` (default: Super). Runtime selection may override.
    pub model: NemotronModel,
}

impl std::fmt::Debug for CoreConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CoreConfig")
            .field("provider", &self.provider)
            .field("nebius_base_url", &self.nebius_base_url)
            .field("has_nebius_api_key", &self.nebius_api_key.is_some())
            .field("has_tavily_api_key", &self.tavily_api_key.is_some())
            .field("recipes_dir", &self.recipes_dir)
            .field("state_log_path", &self.state_log_path)
            .field("model", &self.model.slug())
            .finish()
    }
}

impl CoreConfig {
    pub const DEFAULT_NEBIUS_BASE_URL: &'static str = "https://api.tokenfactory.nebius.com/v1";

    /// Loads configuration from process environment variables.
    /// Automatically loads a `.env` file if present in the current directory.
    pub fn from_env() -> Self {
        Self::load_dotenv_file(PathBuf::from(".env"));
        Self::from_process_env()
    }

    /// Like [`Self::from_env`] but never reads a `.env` file. The desktop app uses this in
    /// release builds, where keys come from the OS keychain.
    pub fn from_process_env() -> Self {
        let provider = env::var("SETUPPAL_PROVIDER")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());

        let nebius_base_url = env::var("NEBIUS_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| Self::DEFAULT_NEBIUS_BASE_URL.to_string());

        let nebius_api_key = env::var("NEBIUS_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());

        let tavily_api_key = env::var("TAVILY_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());

        let recipes_dir = env::var("ORCHESTRATOR_RECIPES_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("recipes"));

        let state_log_path = env::var("ORCHESTRATOR_STATE_LOG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".orchestrator_state.jsonl"));

        let model = env::var("SETUPPAL_MODEL")
            .ok()
            .and_then(|v| NemotronModel::parse(&v))
            .unwrap_or_default();

        Self {
            provider,
            nebius_base_url,
            nebius_api_key,
            tavily_api_key,
            recipes_dir,
            state_log_path,
            model,
        }
    }

    /// Minimal, dependency-free `.env` parser so `.env` works out of the box.
    pub fn load_dotenv_file(path: PathBuf) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_redacts_provider_keys() {
        let config = CoreConfig {
            provider: None,
            nebius_base_url: CoreConfig::DEFAULT_NEBIUS_BASE_URL.to_string(),
            nebius_api_key: Some("test-nebius-secret".to_string()),
            tavily_api_key: Some("test-tavily-secret".to_string()),
            recipes_dir: PathBuf::from("recipes"),
            state_log_path: PathBuf::from(".orchestrator_state.jsonl"),
            model: NemotronModel::Super,
        };

        let debug = format!("{config:?}");
        assert!(!debug.contains("test-nebius-secret"));
        assert!(!debug.contains("test-tavily-secret"));
        assert!(debug.contains("has_nebius_api_key"));
        assert!(debug.contains("super"));
    }
}
