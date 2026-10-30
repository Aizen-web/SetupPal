//! Single source of truth for Nebius Token Factory inference.
//!
//! Chat, planning, health, and key testing all go through [`resolve_provider`], so they can
//! never disagree. The selected Nemotron model comes from [`ModelSelection`]; the Safety Engine
//! never reads it.

use serde::Serialize;

use crate::config::{CoreConfig, CoreError, NemotronModel};

use super::models::ModelSelection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Nebius Token Factory — the only inference backend.
    Nebius,
}

impl ProviderKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "nebius" => Some(Self::Nebius),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        "nebius"
    }

    pub fn display_name(self) -> &'static str {
        "Nebius Token Factory"
    }

    /// Environment variable names that may hold this provider's key, in priority order.
    pub fn key_env_names(self) -> &'static [&'static str] {
        &["NEBIUS_API_KEY"]
    }

    pub fn base_url(self, config: &CoreConfig) -> &str {
        &config.nebius_base_url
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
}

impl ConfigKeyStore {
    pub fn from_config(config: &CoreConfig) -> Self {
        Self {
            nebius: non_empty(config.nebius_api_key.as_deref()),
        }
    }
}

impl std::fmt::Debug for ConfigKeyStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConfigKeyStore")
            .field("has_nebius_key", &self.nebius.is_some())
            .finish()
    }
}

impl KeyStore for ConfigKeyStore {
    fn api_key(&self, kind: ProviderKind) -> Option<String> {
        match kind {
            ProviderKind::Nebius => self.nebius.clone(),
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
    pub model: NemotronModel,
    api_key: String,
}

impl ResolvedProvider {
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn model_id(&self) -> &'static str {
        self.model.api_id()
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
            .field("model", &self.model.api_id())
            .finish_non_exhaustive()
    }
}

/// Nebius is the only provider. An explicit non-empty `SETUPPAL_PROVIDER` other than `nebius`
/// is rejected; unset / blank means Nebius.
pub fn select_provider(requested: Option<&str>) -> Result<ProviderKind, CoreError> {
    match requested.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(ProviderKind::Nebius),
        Some(value) => ProviderKind::parse(value).ok_or_else(|| {
            CoreError::Config(
                "SETUPPAL_PROVIDER must be `nebius` (or left unset). NVIDIA NIM is no longer supported."
                    .to_string(),
            )
        }),
    }
}

pub fn selected_provider(config: &CoreConfig) -> Result<ProviderKind, CoreError> {
    select_provider(config.provider.as_deref())
}

pub fn resolve_provider(
    config: &CoreConfig,
    keys: &dyn KeyStore,
    model: NemotronModel,
) -> Result<ResolvedProvider, CoreError> {
    let kind = selected_provider(config)?;
    let api_key = keys.api_key(kind).ok_or_else(|| {
        CoreError::Config(format!(
            "{} is required. Add {} in Accounts or your local environment.",
            kind.display_name(),
            kind.key_env_names().join(" or ")
        ))
    })?;
    Ok(ResolvedProvider {
        kind,
        base_url: kind.base_url(config).to_string(),
        model,
        api_key,
    })
}

pub fn resolve_with_selection(
    config: &CoreConfig,
    keys: &dyn KeyStore,
    selection: &ModelSelection,
) -> Result<ResolvedProvider, CoreError> {
    resolve_provider(config, keys, selection.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn config(provider: Option<&str>) -> CoreConfig {
        CoreConfig {
            provider: provider.map(str::to_string),
            nebius_base_url: CoreConfig::DEFAULT_NEBIUS_BASE_URL.to_string(),
            nebius_api_key: None,
            tavily_api_key: None,
            recipes_dir: PathBuf::from("recipes"),
            state_log_path: PathBuf::from(".orchestrator_state.jsonl"),
            model: NemotronModel::Super,
        }
    }

    fn keys(nebius: bool) -> ConfigKeyStore {
        ConfigKeyStore {
            nebius: nebius.then(|| "test-nebius-secret".to_string()),
        }
    }

    #[test]
    fn unset_provider_is_always_nebius() {
        let resolved =
            resolve_provider(&config(None), &keys(true), NemotronModel::Super).unwrap();
        assert_eq!(resolved.kind, ProviderKind::Nebius);
        assert_eq!(resolved.model_id(), NemotronModel::Super.api_id());
        assert_eq!(resolved.base_url, CoreConfig::DEFAULT_NEBIUS_BASE_URL);
        assert_eq!(resolved.api_key(), "test-nebius-secret");
    }

    #[test]
    fn selected_model_is_used_without_fallback() {
        let resolved =
            resolve_provider(&config(None), &keys(true), NemotronModel::Ultra).unwrap();
        assert_eq!(resolved.model, NemotronModel::Ultra);
        assert_eq!(resolved.model_id(), NemotronModel::Ultra.api_id());
    }

    #[test]
    fn explicit_nebius_is_accepted() {
        let resolved =
            resolve_provider(&config(Some("  Nebius ")), &keys(true), NemotronModel::Lightning)
                .unwrap();
        assert_eq!(resolved.kind, ProviderKind::Nebius);
        assert_eq!(resolved.model, NemotronModel::Lightning);
    }

    #[test]
    fn missing_key_is_an_error_not_a_fallback() {
        let error =
            resolve_provider(&config(None), &keys(false), NemotronModel::Super).unwrap_err();
        assert!(error.to_string().contains("NEBIUS_API_KEY"));
    }

    #[test]
    fn nim_and_other_providers_are_rejected() {
        assert!(select_provider(Some("nim")).is_err());
        assert!(select_provider(Some("openai")).is_err());
        assert!(resolve_provider(&config(Some("nim")), &keys(true), NemotronModel::Super).is_err());
    }

    #[test]
    fn blank_provider_value_counts_as_unset() {
        assert_eq!(
            select_provider(Some("  ")).unwrap(),
            ProviderKind::Nebius
        );
    }

    #[test]
    fn debug_output_redacts_keys() {
        let resolved =
            resolve_provider(&config(None), &keys(true), NemotronModel::Super).unwrap();
        let debug = format!("{resolved:?} {:?}", keys(true));
        assert!(!debug.contains("test-nebius-secret"));
    }

    #[test]
    fn chat_completions_url_has_a_single_slash() {
        let mut cfg = config(None);
        cfg.nebius_base_url = "https://example.invalid/v1/".to_string();
        let resolved =
            resolve_provider(&cfg, &keys(true), NemotronModel::Super).unwrap();
        assert_eq!(
            resolved.chat_completions_url(),
            "https://example.invalid/v1/chat/completions"
        );
    }
}
