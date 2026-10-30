//! Persisted selection among the allowlisted [`NemotronModel`] values.
//!
//! Arbitrary model IDs are rejected. Selection never influences the Safety Engine.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::config::{CoreError, NemotronModel};
use crate::paths::setuppal_data_dir;

/// Slug used in Settings, prefs, and `SETUPPAL_MODEL`.
pub const DEFAULT_MODEL_SLUG: &str = "super";

/// File under the SetupPal data dir that remembers the user's choice across restarts.
pub const MODEL_PREFS_FILE: &str = "ai_model.json";

#[derive(Debug, Serialize, Deserialize)]
struct ModelPrefsFile {
    model: String,
}

/// Thread-safe selected model, optionally persisted under the SetupPal data dir.
#[derive(Clone)]
pub struct ModelSelection {
    inner: Arc<RwLock<NemotronModel>>,
    persist_path: Option<PathBuf>,
}

impl std::fmt::Debug for ModelSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelSelection")
            .field(
                "model",
                &*self.inner.read().unwrap_or_else(|e| e.into_inner()),
            )
            .field("persist_path", &self.persist_path)
            .finish()
    }
}

impl ModelSelection {
    /// In-memory only (tests / one-shot CLI without a data dir).
    pub fn ephemeral(model: NemotronModel) -> Self {
        Self {
            inner: Arc::new(RwLock::new(model)),
            persist_path: None,
        }
    }

    /// Loads from `SETUPPAL_MODEL` (if set), else the prefs file, else Super.
    pub fn load_default() -> Self {
        let path = setuppal_data_dir()
            .ok()
            .map(|dir| dir.join(MODEL_PREFS_FILE));
        let from_env = std::env::var("SETUPPAL_MODEL")
            .ok()
            .and_then(|v| NemotronModel::parse(&v));
        let from_file = path
            .as_ref()
            .and_then(|p| load_model_prefs(p).ok())
            .flatten();
        let model = from_env.or(from_file).unwrap_or_default();
        Self {
            inner: Arc::new(RwLock::new(model)),
            persist_path: path,
        }
    }

    /// Prefers `preferred` (for example `CoreConfig.model` from env), else the prefs file, else Super.
    pub fn load_with_preferred(preferred: NemotronModel) -> Self {
        let path = setuppal_data_dir()
            .ok()
            .map(|dir| dir.join(MODEL_PREFS_FILE));
        let from_env = std::env::var("SETUPPAL_MODEL")
            .ok()
            .and_then(|v| NemotronModel::parse(&v));
        let from_file = path
            .as_ref()
            .and_then(|p| load_model_prefs(p).ok())
            .flatten();
        // Explicit env wins; otherwise keep a previously saved Settings choice; else config default.
        let model = from_env.or(from_file).unwrap_or(preferred);
        Self {
            inner: Arc::new(RwLock::new(model)),
            persist_path: path,
        }
    }

    pub fn get(&self) -> NemotronModel {
        *self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Sets the model. Never falls back to another model on failure.
    pub fn set(&self, model: NemotronModel) -> Result<(), CoreError> {
        {
            let mut guard = self.inner.write().unwrap_or_else(|e| e.into_inner());
            *guard = model;
        }
        if let Some(path) = &self.persist_path {
            save_model_prefs(path, model)?;
        }
        Ok(())
    }

    pub fn set_from_str(&self, value: &str) -> Result<NemotronModel, CoreError> {
        let model = NemotronModel::parse_or_err(value)?;
        self.set(model)?;
        Ok(model)
    }
}

fn load_model_prefs(path: &Path) -> Result<Option<NemotronModel>, CoreError> {
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    let parsed: ModelPrefsFile = serde_json::from_str(&raw)?;
    Ok(NemotronModel::parse(&parsed.model))
}

fn save_model_prefs(path: &Path, model: NemotronModel) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(&ModelPrefsFile {
        model: model.slug().to_string(),
    })?;
    std::fs::write(path, body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_match_nebius_token_factory() {
        assert_eq!(
            NemotronModel::Lightning.api_id(),
            "nvidia/Nemotron-3_5-Lightning"
        );
        assert_eq!(
            NemotronModel::Super.api_id(),
            "nvidia/nemotron-3-super-120b-a12b"
        );
        assert_eq!(
            NemotronModel::Ultra.api_id(),
            "nvidia/Nemotron-3-Ultra-550b-a55b"
        );
        assert_eq!(NemotronModel::default(), NemotronModel::Super);
        assert_eq!(NemotronModel::default().slug(), DEFAULT_MODEL_SLUG);
    }

    #[test]
    fn parse_accepts_slug_and_api_id_only() {
        assert_eq!(NemotronModel::parse("super"), Some(NemotronModel::Super));
        assert_eq!(
            NemotronModel::parse("nvidia/Nemotron-3_5-Lightning"),
            Some(NemotronModel::Lightning)
        );
        assert_eq!(
            NemotronModel::parse("nvidia/Nemotron-3-Ultra-550b-a55b"),
            Some(NemotronModel::Ultra)
        );
        assert!(NemotronModel::parse("deepseek-ai/DeepSeek-V4-Pro").is_none());
        assert!(NemotronModel::parse("nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B").is_none());
        assert!(NemotronModel::parse("").is_none());
        assert!(NemotronModel::parse_or_err("openai/gpt-oss-120b").is_err());
    }

    #[test]
    fn selection_persists_and_never_silently_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(MODEL_PREFS_FILE);
        let selection = ModelSelection {
            inner: Arc::new(RwLock::new(NemotronModel::Super)),
            persist_path: Some(path.clone()),
        };
        selection.set(NemotronModel::Ultra).unwrap();
        assert_eq!(selection.get(), NemotronModel::Ultra);
        assert!(path.is_file());

        let reloaded = load_model_prefs(&path).unwrap();
        assert_eq!(reloaded, Some(NemotronModel::Ultra));

        assert!(selection.set_from_str("Qwen/Qwen3.5-397B-A17B").is_err());
        assert_eq!(selection.get(), NemotronModel::Ultra);
    }
}
