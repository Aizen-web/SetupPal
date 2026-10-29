//! Single source of truth for which AI provider and model SetupPal talks to.
//!
//! Chat, planning, health, and key testing all go through [`resolve_provider`], so they can
//! never disagree. Each provider is pinned to one model constant in [`CoreConfig`].

use serde::Serialize;

use crate::config::{CoreConfig, CoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Nebius Token Factory (production).
    Nebius,
    /// NVIDIA NIM (testing).
    Nim,
}

impl ProviderKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "nebius" => Some(Self::Nebius),
            "nim" => Some(Self::Nim),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Nebius => "nebius",
            Self::Nim => "nim",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Nebius => "Nebius Token Factory",
            Self::Nim => "NVIDIA NIM",
        }
    }

    pub fn model(self) -> &'static str {
        match self {
            Self::Nebius => CoreConfig::NEBIUS_MODEL,
            Self::Nim => CoreConfig::NIM_MODEL,
        }
    }

    /// Environment variable names that may hold this provider's key, in priority order.
    pub fn key_env_names(self) -> &'static [&'static str] {
        match self {
            Self::Nebius => &["NEBIUS_API_KEY"],
            Self::Nim => &["NIM_API_KEY", "NVIDIA_API_KEY"],
        }
    }

    pub fn base_url(self, config: &CoreConfig) -> &str {
        match self {
            Self::Nebius => &config.nebius_base_url,
            Self::Nim => &config.nim_base_url,
        }
    }
}

/// Where provider keys come from. Implementations must never log or expose key values.
pub trait KeyStore: Send + Sync {
    fn api_key(&self, kind: ProviderKind) -> Option<String>;
}

/// Keys captured in [`CoreConfig`] at startup.
#[derive(Clone, Default)]
pub struct ConfigKeyStore {
    nebius: Option<String>,
    nim: Option<String>,
}

impl ConfigKeyStore {
    pub fn from_config(config: &CoreConfig) -> Self {
        Self {
            nebius: non_empty(config.nebius_api_key.as_deref()),
            nim: non_empty(config.nim_api_key.as_deref()),
        }
    }
}

impl std::fmt::Debug for ConfigKeyStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConfigKeyStore")
            .field("has_nebius_key", &self.nebius.is_some())
            .field("has_nim_key", &self.nim.is_some())
            .finish()
    }
}

impl KeyStore for ConfigKeyStore {
    fn api_key(&self, kind: ProviderKind) -> Option<String> {
        match kind {
            ProviderKind::Nebius => self.nebius.clone(),
            ProviderKind::Nim => self.nim.clone(),
        }
    }
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[derive(Clone)]
pub struct ResolvedProvider {
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: &'static str,
    api_key: String,
}

impl ResolvedProvider {
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }
}

impl std::fmt::Debug for ResolvedProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResolvedProvider")
            .field("kind", &self.kind)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

/// Applies the selection rule: an explicit `SETUPPAL_PROVIDER` wins; otherwise Nebius when a
/// Nebius key exists, else NIM.
pub fn select_provider(
    requested: Option<&str>,
    has_nebius_key: bool,
) -> Result<ProviderKind, CoreError> {
    match requested.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => ProviderKind::parse(value).ok_or_else(|| {
            CoreError::Config(
                "SETUPPAL_PROVIDER must be `nebius` or `nim` (or left unset).".to_string(),
            )
        }),
        None if has_nebius_key => Ok(ProviderKind::Nebius),
        None => Ok(ProviderKind::Nim),
    }
}

/// The provider that would be used, without requiring its key to be present.
pub fn selected_provider(
    config: &CoreConfig,
    keys: &dyn KeyStore,
) -> Result<ProviderKind, CoreError> {
    select_provider(
        config.provider.as_deref(),
        keys.api_key(ProviderKind::Nebius).is_some(),
    )
}

pub fn resolve_provider(
    config: &CoreConfig,
    keys: &dyn KeyStore,
) -> Result<ResolvedProvider, CoreError> {
    let kind = selected_provider(config, keys)?;
    let api_key = keys.api_key(kind).ok_or_else(|| {
        CoreError::Config(format!(
            "{} is selected but no key is set. Add {} to your local environment.",
            kind.display_name(),
            kind.key_env_names().join(" or ")
        ))
    })?;
    Ok(ResolvedProvider {
        kind,
        base_url: kind.base_url(config).to_string(),
        model: kind.model(),
        api_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn config(provider: Option<&str>) -> CoreConfig {
        CoreConfig {
            provider: provider.map(str::to_string),
            nim_base_url: CoreConfig::DEFAULT_NIM_BASE_URL.to_string(),
            nim_api_key: None,
            nebius_base_url: CoreConfig::DEFAULT_NEBIUS_BASE_URL.to_string(),
            nebius_api_key: None,
            tavily_api_key: None,
            recipes_dir: PathBuf::from("recipes"),
            state_log_path: PathBuf::from(".orchestrator_state.jsonl"),
        }
    }

    fn keys(nebius: bool, nim: bool) -> ConfigKeyStore {
        ConfigKeyStore {
            nebius: nebius.then(|| "test-nebius-secret".to_string()),
            nim: nim.then(|| "test-nim-secret".to_string()),
        }
    }

    #[test]
    fn unset_provider_prefers_nebius_when_its_key_exists() {
        let resolved = resolve_provider(&config(None), &keys(true, true)).unwrap();
        assert_eq!(resolved.kind, ProviderKind::Nebius);
        assert_eq!(resolved.model, CoreConfig::NEBIUS_MODEL);
        assert_eq!(resolved.base_url, CoreConfig::DEFAULT_NEBIUS_BASE_URL);
        assert_eq!(resolved.api_key(), "test-nebius-secret");
    }

    #[test]
    fn unset_provider_falls_back_to_nim_without_a_nebius_key() {
        let resolved = resolve_provider(&config(None), &keys(false, true)).unwrap();
        assert_eq!(resolved.kind, ProviderKind::Nim);
        assert_eq!(resolved.model, CoreConfig::NIM_MODEL);
    }

    #[test]
    fn explicit_nim_wins_even_when_both_keys_exist() {
        let resolved = resolve_provider(&config(Some("nim")), &keys(true, true)).unwrap();
        assert_eq!(resolved.kind, ProviderKind::Nim);
        assert_eq!(resolved.model, CoreConfig::NIM_MODEL);
        assert_eq!(resolved.api_key(), "test-nim-secret");
    }

    #[test]
    fn explicit_choice_is_case_and_whitespace_insensitive() {
        let resolved = resolve_provider(&config(Some("  Nebius ")), &keys(true, false)).unwrap();
        assert_eq!(resolved.kind, ProviderKind::Nebius);
    }

    #[test]
    fn explicit_provider_without_its_key_is_an_error_not_a_fallback() {
        let error = resolve_provider(&config(Some("nebius")), &keys(false, true)).unwrap_err();
        assert!(error.to_string().contains("NEBIUS_API_KEY"));
    }

    #[test]
    fn unknown_provider_value_is_rejected() {
        assert!(select_provider(Some("openai"), true).is_err());
        assert!(resolve_provider(&config(Some("openai")), &keys(true, true)).is_err());
    }

    #[test]
    fn blank_provider_value_counts_as_unset() {
        assert_eq!(select_provider(Some("  "), false).unwrap(), ProviderKind::Nim);
    }

    #[test]
    fn no_keys_reports_the_selected_provider_and_its_key_names() {
        let error = resolve_provider(&config(None), &keys(false, false)).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("NIM_API_KEY"));
        assert!(message.contains("NVIDIA_API_KEY"));
    }

    #[test]
    fn debug_output_redacts_keys() {
        let resolved = resolve_provider(&config(None), &keys(true, true)).unwrap();
        let debug = format!("{resolved:?} {:?}", keys(true, true));
        assert!(!debug.contains("test-nebius-secret"));
        assert!(!debug.contains("test-nim-secret"));
    }

    #[test]
    fn chat_completions_url_has_a_single_slash() {
        let mut cfg = config(Some("nim"));
        cfg.nim_base_url = "https://example.invalid/v1/".to_string();
        let resolved = resolve_provider(&cfg, &keys(false, true)).unwrap();
        assert_eq!(
            resolved.chat_completions_url(),
            "https://example.invalid/v1/chat/completions"
        );
    }
}
