use std::env;
use std::path::PathBuf;
use thiserror::Error;

/// Looks up a secret (for example a Tavily or GitHub key) at the moment it is needed, so a key
/// saved while the app is running takes effect without a restart. Never log what it returns.
pub type SecretProvider = std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>;

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
    /// Raw `SETUPPAL_PROVIDER` value (`nebius` or `nim`); validated by `llm::provider::select_provider`.
    pub provider: Option<String>,
    pub nim_base_url: String,
    pub nim_api_key: Option<String>,
    pub nebius_base_url: String,
    pub nebius_api_key: Option<String>,
    pub tavily_api_key: Option<String>,
    pub recipes_dir: PathBuf,
    pub state_log_path: PathBuf,
}

impl std::fmt::Debug for CoreConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CoreConfig")
            .field("provider", &self.provider)
            .field("nim_base_url", &self.nim_base_url)
            .field("has_nim_api_key", &self.nim_api_key.is_some())
            .field("nebius_base_url", &self.nebius_base_url)
            .field("has_nebius_api_key", &self.nebius_api_key.is_some())
            .field("has_tavily_api_key", &self.tavily_api_key.is_some())
            .field("recipes_dir", &self.recipes_dir)
            .field("state_log_path", &self.state_log_path)
            .finish()
    }
}

impl CoreConfig {
    pub const DEFAULT_NIM_BASE_URL: &'static str = "https://integrate.api.nvidia.com/v1";
    /// Pinned NVIDIA NIM model (testing provider). Not configurable.
    pub const NIM_MODEL: &'static str = "nvidia/nemotron-3.5-bolt-e4b";
    pub const DEFAULT_NEBIUS_BASE_URL: &'static str = "https://api.tokenfactory.nebius.com/v1";
    /// Pinned Nebius Token Factory model (production provider). Not configurable.
    pub const NEBIUS_MODEL: &'static str = "nvidia/Nemotron-3_5-Lightning";

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

        let nim_base_url = env::var("NIM_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| Self::DEFAULT_NIM_BASE_URL.to_string());

        let nim_api_key = ["NIM_API_KEY", "NVIDIA_API_KEY"]
            .iter()
            .find_map(|name| env::var(name).ok().filter(|value| !value.trim().is_empty()));

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

        Self {
            provider,
            nim_base_url,
            nim_api_key,
            nebius_base_url,
            nebius_api_key,
            tavily_api_key,
            recipes_dir,
            state_log_path,
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
            nim_base_url: CoreConfig::DEFAULT_NIM_BASE_URL.to_string(),
            nim_api_key: Some("test-nim-secret".to_string()),
            nebius_base_url: CoreConfig::DEFAULT_NEBIUS_BASE_URL.to_string(),
            nebius_api_key: Some("test-nebius-secret".to_string()),
            tavily_api_key: Some("test-tavily-secret".to_string()),
            recipes_dir: PathBuf::from("recipes"),
            state_log_path: PathBuf::from(".orchestrator_state.jsonl"),
        };

        let debug = format!("{config:?}");
        assert!(!debug.contains("test-nim-secret"));
        assert!(!debug.contains("test-nebius-secret"));
        assert!(!debug.contains("test-tavily-secret"));
        assert!(debug.contains("has_nebius_api_key"));
    }
}
