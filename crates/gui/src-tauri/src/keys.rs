//! Key storage for SetupPal.
//!
//! Keys live in the operating system keychain (Windows Credential Manager) under the service
//! name `SetupPal`. Nothing here ever logs, returns, or masks a key value.
//!
//! Resolution order for every lookup: keychain, then process environment, then a `.env` file.
//! The `.env` file is read only in debug builds or when `SETUPPAL_DEV=1`.

#[cfg(test)]
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use installer_core::llm::{KeyStore, ProviderKind};
use installer_core::SecretProvider;
use tracing::warn;

/// Credential Manager service name.
pub const SERVICE: &str = "SetupPal";

/// Longest value accepted by `POST /api/keys`.
pub const MAX_KEY_LEN: usize = 4096;

/// The only secrets SetupPal stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KeyName {
    NebiusApiKey,
    NimApiKey,
    TavilyApiKey,
    GithubToken,
}

impl KeyName {
    pub const ALL: [KeyName; 4] = [
        KeyName::NebiusApiKey,
        KeyName::NimApiKey,
        KeyName::TavilyApiKey,
        KeyName::GithubToken,
    ];

    /// Account name in the keychain and name used by the HTTP API.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NebiusApiKey => "nebius_api_key",
            Self::NimApiKey => "nim_api_key",
            Self::TavilyApiKey => "tavily_api_key",
            Self::GithubToken => "github_token",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == name)
    }

    /// Environment variables that may hold this secret, in priority order.
    pub fn env_names(self) -> &'static [&'static str] {
        match self {
            Self::NebiusApiKey => &["NEBIUS_API_KEY"],
            Self::NimApiKey => &["NIM_API_KEY", "NVIDIA_API_KEY"],
            Self::TavilyApiKey => &["TAVILY_API_KEY"],
            Self::GithubToken => &["GITHUB_TOKEN", "GH_TOKEN"],
        }
    }

    pub fn for_provider(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::Nebius => Self::NebiusApiKey,
            ProviderKind::Nim => Self::NimApiKey,
        }
    }
}

/// Storage failure. Never carries a key value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyError(pub String);

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for KeyError {}

/// A place secrets can be saved. `get` of a missing entry is `Ok(None)`, and deleting a missing
/// entry is `Ok(())`.
pub trait SecretStore: Send + Sync {
    fn get(&self, name: KeyName) -> Result<Option<String>, KeyError>;
    fn set(&self, name: KeyName, value: &str) -> Result<(), KeyError>;
    fn delete(&self, name: KeyName) -> Result<(), KeyError>;
}

/// The OS keychain.
pub struct KeyringStore;

impl KeyringStore {
    fn entry(name: KeyName) -> Result<keyring::Entry, KeyError> {
        keyring::Entry::new(SERVICE, name.as_str())
            .map_err(|e| KeyError(format!("Could not open the OS keychain: {e}")))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, name: KeyName) -> Result<Option<String>, KeyError> {
        match Self::entry(name)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(KeyError(format!(
                "Could not read from the OS keychain: {e}"
            ))),
        }
    }

    fn set(&self, name: KeyName, value: &str) -> Result<(), KeyError> {
        Self::entry(name)?
            .set_password(value)
            .map_err(|e| KeyError(format!("Could not save to the OS keychain: {e}")))
    }

    fn delete(&self, name: KeyName) -> Result<(), KeyError> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(KeyError(format!(
                "Could not remove from the OS keychain: {e}"
            ))),
        }
    }
}

/// In-memory store used by tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore {
    values: std::sync::Mutex<BTreeMap<KeyName, String>>,
}

#[cfg(test)]
impl SecretStore for MemoryStore {
    fn get(&self, name: KeyName) -> Result<Option<String>, KeyError> {
        Ok(self.values.lock().unwrap().get(&name).cloned())
    }

    fn set(&self, name: KeyName, value: &str) -> Result<(), KeyError> {
        self.values.lock().unwrap().insert(name, value.to_string());
        Ok(())
    }

    fn delete(&self, name: KeyName) -> Result<(), KeyError> {
        self.values.lock().unwrap().remove(&name);
        Ok(())
    }
}

/// Cleans a pasted or `.env` value; returns `None` for empty values and the example placeholder.
pub fn normalize_api_key(raw: &str) -> Option<String> {
    let clean = raw
        .trim()
        .trim_start_matches('\u{feff}')
        .trim_matches('"')
        .trim_matches('\'')
        .trim();
    if clean.is_empty() || clean == "nvapi-REPLACE_WITH_YOUR_KEY" {
        None
    } else {
        Some(clean.to_string())
    }
}

/// Checks a value before it is stored. The error text never includes the value.
pub fn validate_value(raw: &str) -> Result<String, &'static str> {
    let value = normalize_api_key(raw).ok_or("The key is empty.")?;
    if value.len() > MAX_KEY_LEN {
        return Err("The key is too long.");
    }
    if value.chars().any(char::is_control) {
        return Err("The key contains control characters.");
    }
    Ok(value)
}

/// Resolves secrets: keychain, then environment, then (developer only) `.env`.
pub struct Keys {
    store: Arc<dyn SecretStore>,
    dev_fallback: bool,
    dotenv_path: Option<PathBuf>,
}

impl Keys {
    pub fn new(store: Arc<dyn SecretStore>, dev_fallback: bool) -> Self {
        let dotenv_path = dev_fallback.then(find_dotenv).flatten();
        Self {
            store,
            dev_fallback,
            dotenv_path,
        }
    }

    /// Test constructor that never looks at the real `.env` file.
    #[cfg(test)]
    pub fn without_dotenv(store: Arc<dyn SecretStore>) -> Self {
        Self {
            store,
            dev_fallback: false,
            dotenv_path: None,
        }
    }

    #[cfg(test)]
    pub fn with_dotenv(store: Arc<dyn SecretStore>, path: PathBuf) -> Self {
        Self {
            store,
            dev_fallback: true,
            dotenv_path: Some(path),
        }
    }

    pub fn dev_fallback(&self) -> bool {
        self.dev_fallback
    }

    pub fn dotenv_path(&self) -> Option<&PathBuf> {
        self.dotenv_path.as_ref()
    }

    pub fn resolve(&self, name: KeyName) -> Option<String> {
        self.read_keychain(name)
            .or_else(|| self.read_environment(name))
            .or_else(|| self.read_dotenv(name))
    }

    fn read_keychain(&self, name: KeyName) -> Option<String> {
        match self.store.get(name) {
            Ok(value) => value.as_deref().and_then(normalize_api_key),
            Err(error) => {
                warn!(key = name.as_str(), "keychain read failed: {error}");
                None
            }
        }
    }

    fn read_environment(&self, name: KeyName) -> Option<String> {
        name.env_names()
            .iter()
            .find_map(|var| std::env::var(var).ok().and_then(|v| normalize_api_key(&v)))
    }

    fn read_dotenv(&self, name: KeyName) -> Option<String> {
        let path = self.dotenv_path.as_ref()?;
        let content = std::fs::read_to_string(path).ok()?;
        name.env_names()
            .iter()
            .find_map(|var| dotenv_value(&content, var))
    }

    pub fn save(&self, name: KeyName, value: &str) -> Result<(), KeyError> {
        self.store.set(name, value)
    }

    pub fn remove(&self, name: KeyName) -> Result<(), KeyError> {
        self.store.delete(name)
    }

    /// Booleans only: whether each key is usable, and whether it is stored in the keychain.
    pub fn status(&self) -> serde_json::Value {
        let mut configured = serde_json::Map::new();
        let mut in_keychain = serde_json::Map::new();
        for name in KeyName::ALL {
            configured.insert(
                name.as_str().to_string(),
                self.resolve(name).is_some().into(),
            );
            in_keychain.insert(
                name.as_str().to_string(),
                self.read_keychain(name).is_some().into(),
            );
        }
        serde_json::json!({ "configured": configured, "in_keychain": in_keychain })
    }

    /// A lookup closure for the core crate, evaluated at each use.
    pub fn provider(self: &Arc<Self>, name: KeyName) -> SecretProvider {
        let keys = Arc::clone(self);
        Arc::new(move || keys.resolve(name))
    }
}

impl KeyStore for Keys {
    fn api_key(&self, kind: ProviderKind) -> Option<String> {
        self.resolve(KeyName::for_provider(kind))
    }
}

/// Looks for `.env` in the working directory and a few parents (`tauri dev` runs from `src-tauri`).
fn find_dotenv() -> Option<PathBuf> {
    let start = std::env::current_dir().ok()?;
    start
        .ancestors()
        .take(5)
        .map(|dir| dir.join(".env"))
        .find(|p| p.is_file())
}

fn dotenv_value(content: &str, name: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        if let Some(value) = line.strip_prefix(&format!("{name}=")) {
            if let Some(clean) = normalize_api_key(value) {
                return Some(clean);
            }
        }
    }
    None
}

/// Developer fallback switch: debug builds, or `SETUPPAL_DEV=1`.
pub fn dev_fallback_enabled() -> bool {
    cfg!(debug_assertions) || std::env::var("SETUPPAL_DEV").is_ok_and(|v| v.trim() == "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> Arc<MemoryStore> {
        Arc::new(MemoryStore::default())
    }

    #[test]
    fn names_round_trip_and_unknown_names_are_rejected() {
        for name in KeyName::ALL {
            assert_eq!(KeyName::parse(name.as_str()), Some(name));
        }
        assert_eq!(
            KeyName::parse("tavily_api_key"),
            Some(KeyName::TavilyApiKey)
        );
        assert_eq!(KeyName::parse("../etc"), None);
        assert_eq!(KeyName::parse(""), None);
    }

    #[test]
    fn store_save_read_remove_and_remove_is_idempotent() {
        let store = memory();
        let keys = Keys::without_dotenv(store.clone());
        assert_eq!(keys.resolve(KeyName::TavilyApiKey), None);
        keys.save(KeyName::TavilyApiKey, "tvly-test-value").unwrap();
        assert_eq!(
            keys.resolve(KeyName::TavilyApiKey).as_deref(),
            Some("tvly-test-value")
        );
        keys.remove(KeyName::TavilyApiKey).unwrap();
        keys.remove(KeyName::TavilyApiKey).unwrap();
        assert_eq!(keys.resolve(KeyName::TavilyApiKey), None);
    }

    #[test]
    fn provider_closure_sees_keys_saved_after_it_was_created() {
        let keys = Arc::new(Keys::without_dotenv(memory()));
        let provider = keys.provider(KeyName::GithubToken);
        assert_eq!(provider(), None);
        keys.save(KeyName::GithubToken, "ghp_late").unwrap();
        assert_eq!(provider().as_deref(), Some("ghp_late"));
    }

    #[test]
    fn keychain_wins_over_environment_and_dotenv() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(&path, "TAVILY_API_KEY=from-dotenv\n").unwrap();
        let keys = Keys::with_dotenv(memory(), path);
        if std::env::var_os("TAVILY_API_KEY").is_none() {
            assert_eq!(
                keys.resolve(KeyName::TavilyApiKey).as_deref(),
                Some("from-dotenv")
            );
        }
        keys.save(KeyName::TavilyApiKey, "from-keychain").unwrap();
        assert_eq!(
            keys.resolve(KeyName::TavilyApiKey).as_deref(),
            Some("from-keychain")
        );
    }

    #[test]
    fn dotenv_is_ignored_when_the_fallback_is_off() {
        let keys = Keys::without_dotenv(memory());
        assert!(keys.dotenv_path().is_none());
        assert!(!keys.dev_fallback());
        assert_eq!(keys.resolve(KeyName::NebiusApiKey), None);
    }

    #[test]
    fn dotenv_parser_handles_quotes_export_bom_and_placeholders() {
        let content = "\u{feff}# comment\nexport NIM_API_KEY=\"quoted\"\nNEBIUS_API_KEY=nvapi-REPLACE_WITH_YOUR_KEY\nOTHER=1\n";
        assert_eq!(
            dotenv_value(content, "NIM_API_KEY").as_deref(),
            Some("quoted")
        );
        assert_eq!(dotenv_value(content, "NEBIUS_API_KEY"), None);
        assert_eq!(dotenv_value(content, "MISSING"), None);
    }

    #[test]
    fn status_reports_booleans_only() {
        let keys = Keys::without_dotenv(memory());
        keys.save(KeyName::NimApiKey, "nvapi-super-secret").unwrap();
        let status = keys.status();
        assert_eq!(status["configured"]["nim_api_key"], true);
        assert_eq!(status["in_keychain"]["nim_api_key"], true);
        assert_eq!(status["configured"]["github_token"], false);
        assert!(!status.to_string().contains("super-secret"));
        assert!(status["configured"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v.is_boolean()));
    }

    #[test]
    fn provider_keys_come_from_the_store() {
        let keys = Keys::without_dotenv(memory());
        keys.save(KeyName::NebiusApiKey, "neb-key").unwrap();
        assert_eq!(
            keys.api_key(ProviderKind::Nebius).as_deref(),
            Some("neb-key")
        );
        assert_eq!(keys.api_key(ProviderKind::Nim), None);
    }

    #[test]
    fn validation_rejects_empty_long_and_control_values_without_echoing() {
        assert_eq!(validate_value("  \"abc\"  ").unwrap(), "abc");
        assert!(validate_value("   ").is_err());
        assert!(validate_value(&"x".repeat(MAX_KEY_LEN + 1)).is_err());
        let err = validate_value("bad\u{0007}secret").unwrap_err();
        assert!(!err.contains("secret"));
    }
}
