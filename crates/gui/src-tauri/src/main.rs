//! SetupPal desktop shell and zero-trust backend.
//!
//! One Tauri process: it starts the Axum API on `127.0.0.1:<random port>`, generates a per-launch
//! bearer token, and hands both to the webview through the `backend_info` command.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod intent;
mod keys;

use std::collections::HashMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Path as AxumPath, State},
    http::{header, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tokio_stream::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

use installer_core::llm::{ProviderClient, ProviderKind, ResolvedProvider};
use installer_core::{
    apply_to_safety_report, check_plan, first_manual, is_broad_request,
    manual_guide_with_installed, AllowedBinary, CoreConfig,
    EventBus, ExecutionOptions, FailureContext, Healer, LlmClient, OsProvider, Plan,
    PlanSafetyReport, Planner, PlatformOsProvider, RecipeStore, RiskRating, SafetyEngine,
    StateLogger, SystemProfile, SystemScanner, TrustCategory,
};
use intent::{IntentStreamFilter, SetupIntent};
use keys::{normalize_api_key, KeyName, Keys};

#[derive(Clone)]
pub struct AppState {
    pub config: CoreConfig,
    pub events: EventBus,
    pub os_provider: Arc<dyn OsProvider>,
    pub scanner: Arc<SystemScanner>,
    pub safety_engine: Arc<SafetyEngine>,
    pub recipes: RecipeStore,
    /// Shared by chat and the planner so both always use the same provider and pinned model.
    pub llm: Arc<ProviderClient>,
    pub planner: Arc<Planner>,
    pub state_logger: StateLogger,
    /// Keychain-backed secrets (provider keys, Tavily, GitHub). Values never leave the backend.
    pub keys: Arc<Keys>,
    /// Server-side plans keyed by `plan_id` (never trust a client-supplied plan body).
    pub plans: Arc<RwLock<HashMap<String, (Plan, PlanSafetyReport)>>>,
    /// Active execute runs keyed by client `run_id`.
    pub runs: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// Last safety level reported by a client (for `/api/health`).
    pub safety_level: Arc<RwLock<String>>,
    /// Only one install or uninstall may run at a time across the app.
    pub install_lock: Arc<tokio::sync::Mutex<()>>,
}

/// Address and token the webview needs. Delivered by the `backend_info` command only.
#[derive(Clone, Serialize)]
struct BackendInfo {
    api_base: String,
    token: String,
}

// Not derived: a `Debug` impl on this type would put the token in logs.
impl std::fmt::Debug for BackendInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackendInfo")
            .field("api_base", &self.api_base)
            .finish_non_exhaustive()
    }
}

#[tauri::command]
fn backend_info(info: tauri::State<'_, BackendInfo>) -> BackendInfo {
    info.inner().clone()
}

/// Builds the shared state. Debug builds (or `SETUPPAL_DEV=1`) also read `.env` as a developer fallback.
fn build_app_state(keys: Arc<Keys>) -> anyhow::Result<AppState> {
    let mut config = if keys.dev_fallback() {
        if let Some(path) = keys.dotenv_path() {
            CoreConfig::load_dotenv_file(path.clone());
        }
        CoreConfig::from_process_env()
    } else {
        CoreConfig::from_process_env()
    };
    // An installed app has no meaningful working directory, so keep its history in the user's
    // data folder. Developer runs and an explicit ORCHESTRATOR_STATE_LOG keep the old path.
    if !keys.dev_fallback() && std::env::var_os("ORCHESTRATOR_STATE_LOG").is_none() {
        let dir = installer_core::paths::setuppal_data_dir()?;
        std::fs::create_dir_all(&dir)?;
        config.state_log_path = dir.join("state.jsonl");
    }

    let events = EventBus::default();
    let os_provider: Arc<dyn OsProvider> = Arc::new(PlatformOsProvider::new());
    let scanner = Arc::new(SystemScanner::new(os_provider.clone(), events.clone()));
    let safety_engine = Arc::new(SafetyEngine::with_secret_providers(
        os_provider.clone(),
        keys.provider(KeyName::TavilyApiKey),
        keys.provider(KeyName::GithubToken),
        events.clone(),
    ));
    let recipes = RecipeStore::load_from_dir_or_embedded(&config.recipes_dir)?;
    let llm = Arc::new(ProviderClient::with_key_store(
        config.clone(),
        keys.clone(),
    )?);
    let planner = Arc::new(Planner::new(
        recipes.clone(),
        llm.clone(),
        events.clone(),
        os_provider.clone(),
    ));
    let state_logger = StateLogger::new(config.state_log_path.clone(), events.clone());

    Ok(AppState {
        config,
        events,
        os_provider,
        scanner,
        safety_engine,
        recipes,
        llm,
        planner,
        state_logger,
        keys,
        plans: Arc::new(RwLock::new(HashMap::new())),
        runs: Arc::new(Mutex::new(HashMap::new())),
        safety_level: Arc::new(RwLock::new("strict".to_string())),
        install_lock: Arc::new(tokio::sync::Mutex::new(())),
    })
}

/// Every API route, before auth is applied.
fn api_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/doctor", get(doctor_handler))
        .route("/api/recipes", get(recipes_handler))
        .route("/api/chat/stream", post(chat_stream_handler))
        .route("/api/plan", post(plan_handler))
        .route("/api/execute", post(execute_handler))
        .route("/api/execute/cancel", post(cancel_execute_handler))
        .route("/api/history", get(history_handler))
        .route("/api/history/export", get(history_export_handler))
        .route("/api/undo", post(undo_handler))
        .route("/api/events", get(events_handler))
        .route("/api/test-key", post(test_provider_key_handler))
        .route("/api/keys", post(save_key_handler))
        .route("/api/keys/status", get(key_status_handler))
        .route("/api/keys/:name", delete(delete_key_handler))
        .with_state(state)
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,installer_gui=debug,installer_core=debug".into()),
        )
        .init();

    tauri::Builder::default()
        // Must be registered first: a second launch hands control to the running window and exits,
        // so two copies never fight over the keychain, the state log, or each other's plans.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![backend_info])
        .setup(|app| {
            use tauri::Manager;
            info!("Initializing SetupPal Zero-Trust Core...");

            let keys = Arc::new(Keys::new(
                Arc::new(keys::KeyringStore),
                keys::dev_fallback_enabled(),
            ));
            let state = build_app_state(keys)?;

            let listener =
                tauri::async_runtime::block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))?;
            let port = listener.local_addr()?.port();
            let token = auth::generate_token()
                .map_err(|e| anyhow::anyhow!("no secure random source: {e}"))?;

            let router = auth::secure(
                api_router(state),
                auth::AuthState::new(token.clone(), port),
                cfg!(debug_assertions),
            );
            // The token is deliberately absent from this log line.
            info!("SetupPal backend listening on 127.0.0.1:{port}");
            tauri::async_runtime::spawn(async move {
                if let Err(e) = axum::serve(listener, router).await {
                    error!("Axum server error: {:?}", e);
                }
            });

            app.manage(BackendInfo {
                api_base: format!("http://127.0.0.1:{port}"),
                token,
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running SetupPal");
}

/// Provider fields for `/api/health`, from the same resolver chat and planning use.
fn provider_status(llm: &ProviderClient) -> serde_json::Value {
    let keys = llm.key_store();
    let (provider, model, error) = match llm.selected() {
        Ok(kind) => {
            let error = llm.resolve().err().map(|e| e.to_string());
            (Some(kind.id()), Some(kind.model()), error)
        }
        Err(e) => (None, None, Some(e.to_string())),
    };
    serde_json::json!({
        "provider": provider,
        "model": model,
        "provider_ready": error.is_none(),
        "provider_error": error,
        "has_nebius_key": keys.api_key(ProviderKind::Nebius).is_some(),
        "has_nim_key": keys.api_key(ProviderKind::Nim).is_some(),
    })
}

async fn health_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let has_tavily_key = state.keys.resolve(KeyName::TavilyApiKey).is_some();
    let has_github_token = state.keys.resolve(KeyName::GithubToken).is_some();

    let safety_level = state.safety_level.read().await.clone();
    let mut health = serde_json::json!({
        "status": "ok",
        "app": "SetupPal Zero-Trust Backend",
        "version": "0.1.0",
        "safety_level": safety_level,
        "deterministic_gates": true,
        "has_tavily_key": has_tavily_key,
        "has_github_token": has_github_token,
    });
    if let (Some(target), serde_json::Value::Object(fields)) =
        (health.as_object_mut(), provider_status(&state.llm))
    {
        target.extend(fields);
    }
    Json(health)
}

async fn doctor_handler(
    State(state): State<AppState>,
) -> Result<Json<SystemProfile>, (StatusCode, String)> {
    let profile = state
        .scanner
        .scan()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(profile))
}

async fn recipes_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let recipes = state.recipes.list();
    Json(serde_json::json!({
        "count": recipes.len(),
        "recipes": recipes,
    }))
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessageItem {
    pub role: String,
    pub content: String,
}

/// Provider, model, and key are never taken from the client; unknown fields are ignored.
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub history: Option<Vec<ChatMessageItem>>,
    /// How many recent messages are sent word for word (older ones are compacted into notes).
    pub memory_turns: Option<usize>,
    /// Max tokens for the reply.
    pub max_reply_tokens: Option<u32>,
    /// Whether to attach PC facts to the message.
    pub include_pc_facts: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinecraftInspection {
    pub java_edition_installed: bool,
    pub java_edition_path: Option<String>,
    pub bedrock_edition_installed: bool,
    pub bedrock_edition_path: Option<String>,
    pub installed_versions: Vec<String>,
    pub java_runtime_version: Option<String>,
    pub java_executable_path: Option<String>,
    pub gpu_detected: Option<String>,
}

pub fn inspect_minecraft_system(profile: &SystemProfile) -> MinecraftInspection {
    let mut java_edition_installed = false;
    let mut java_edition_path = None;
    let mut bedrock_edition_installed = false;
    let mut bedrock_edition_path = None;
    let mut installed_versions = Vec::new();

    // Check %APPDATA%\.minecraft
    if let Ok(appdata) = std::env::var("APPDATA") {
        let mc_dir = PathBuf::from(appdata).join(".minecraft");
        if mc_dir.exists() {
            java_edition_installed = true;
            let versions_dir = mc_dir.join("versions");
            if versions_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(versions_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if !name.ends_with(".json") && !name.ends_with(".dat") {
                            installed_versions.push(name);
                        }
                    }
                }
            }
            java_edition_path = Some(mc_dir.to_string_lossy().to_string());
        }
    }

    // Check Bedrock (%LOCALAPPDATA%\Packages\Microsoft.MinecraftUWP_*)
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        let packages_dir = PathBuf::from(localappdata).join("Packages");
        if packages_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(packages_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("Microsoft.MinecraftUWP_") {
                        bedrock_edition_installed = true;
                        bedrock_edition_path = Some(entry.path().to_string_lossy().to_string());
                        break;
                    }
                }
            }
        }
    }

    // Check Java tool from SystemProfile
    let (java_runtime_version, java_executable_path) = profile
        .installed_tools
        .iter()
        .find(|t| t.binary == AllowedBinary::Java)
        .map(|t| {
            (
                Some(t.version_output.clone()),
                Some(t.path.to_string_lossy().to_string()),
            )
        })
        .unwrap_or((None, None));

    MinecraftInspection {
        java_edition_installed,
        java_edition_path,
        bedrock_edition_installed,
        bedrock_edition_path,
        installed_versions,
        java_runtime_version,
        java_executable_path,
        gpu_detected: profile.gpu_detected.clone(),
    }
}

pub fn create_metadata_for_plan(
    plan: &Plan,
    safety_report: &PlanSafetyReport,
) -> Vec<serde_json::Value> {
    plan.steps
        .iter()
        .map(|s| {
            let (domain, url) = s.step.source_display();
            let report = safety_report
                .step_reports
                .iter()
                .find(|r| r.step_id == s.id);
            let (verdict_title, verdict_desc) = report
                .map(|r| r.user_facing_translation())
                .unwrap_or(("Verified source", "Passed security checks.".to_string()));
            let resolved_version = match &s.step {
                installer_core::Step::InstallPackage { version, .. } => version.clone(),
                _ => None,
            };
            serde_json::json!({
                "step_id": s.id,
                "source_domain": domain,
                "source_url": url,
                "verdict_title": verdict_title,
                "verdict_desc": verdict_desc,
                "resolved_version": resolved_version,
            })
        })
        .collect()
}

/// Fills install-step versions from adapter metadata when the plan left them unset (R23).
async fn enrich_plan_versions(plan: &mut Plan, state: &AppState) {
    let adapters = installer_core::AdapterRegistry::with_system_defaults(state.os_provider.clone());
    for step in &mut plan.steps {
        let installer_core::Step::InstallPackage {
            package_id,
            version,
            source_adapter,
            ..
        } = &mut step.step
        else {
            continue;
        };
        if version.as_ref().is_some_and(|v| !v.is_empty() && v != "latest") {
            // Keep an explicit version in the step label.
            if !step.description.contains(version.as_deref().unwrap_or("")) {
                if let Some(v) = version.as_deref() {
                    step.description =
                        format_install_label(&step.description, package_id, v, *source_adapter);
                }
            }
            continue;
        }
        let Some(adapter) = adapters.get(*source_adapter) else {
            continue;
        };
        if let Ok(meta) = adapter.resolve_metadata(package_id, None).await {
            if !meta.resolved_version.is_empty() && meta.resolved_version != "latest" {
                *version = Some(meta.resolved_version.clone());
                step.description = format_install_label(
                    &step.description,
                    package_id,
                    &meta.resolved_version,
                    *source_adapter,
                );
            }
        }
    }
}

fn format_install_label(
    previous: &str,
    package_id: &str,
    version: &str,
    adapter: installer_core::AdapterKind,
) -> String {
    let eco = match adapter {
        installer_core::AdapterKind::Winget => "winget",
        installer_core::AdapterKind::Npm => "npm",
        installer_core::AdapterKind::PyPI => "pip",
        installer_core::AdapterKind::Cargo => "cargo",
        _ => "package",
    };
    let name = if previous.to_ascii_lowercase().starts_with("install ") {
        // Prefer a human title already in the description over a raw id.
        let rest = previous["Install ".len()..].trim();
        let cut = rest
            .find(" (")
            .or_else(|| rest.find(char::is_numeric))
            .unwrap_or(rest.len());
        let candidate = rest[..cut].trim();
        if candidate.is_empty() {
            package_id
        } else {
            candidate
        }
    } else {
        package_id
    };
    format!("Install {name} {version} ({eco})")
}

fn parse_confirm_validation(msg: &str) -> Option<serde_json::Value> {
    // "plan validation error: confirm:PyPI:reqeusts:requests"
    let idx = msg.find("confirm:")?;
    let rest = &msg[idx + "confirm:".len()..];
    let mut parts = rest.splitn(3, ':');
    let eco = parts.next()?.trim();
    let typed = parts.next()?.trim();
    let suggestion = parts.next()?.trim();
    if typed.is_empty() || suggestion.is_empty() {
        return None;
    }
    let eco_l = eco.to_ascii_lowercase();
    let ecosystem = match eco_l.as_str() {
        "pypi" => "pypi",
        "npm" => "npm",
        "winget" => "winget",
        _ => eco_l.as_str(),
    };
    Some(serde_json::json!({
        "kind": "confirm",
        "typed": typed,
        "suggestion": suggestion,
        "ecosystem": ecosystem,
    }))
}

#[derive(Debug, Deserialize)]
pub struct TestKeyRequest {
    /// A pasted key to test once; empty means "test the configured key".
    #[serde(default)]
    pub api_key: String,
    /// `nebius` or `nim`; defaults to the provider chat currently uses.
    pub provider: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TestKeyResponse {
    pub valid: bool,
    pub message: String,
    pub model_tested: String,
    pub provider: String,
}

/// Tests a key with a one-token request to the provider's pinned model, so a pass means
/// chat will work with that key. Provider response bodies are never echoed.
async fn test_provider_key_handler(
    State(state): State<AppState>,
    Json(req): Json<TestKeyRequest>,
) -> Json<TestKeyResponse> {
    if let Some(service @ ("tavily" | "github")) = req.provider.as_deref() {
        return test_service_key(&state, service, &req.api_key).await;
    }
    let kind = match req.provider.as_deref() {
        Some(value) => ProviderKind::parse(value),
        None => state.llm.selected().ok(),
    };
    let Some(kind) = kind else {
        return Json(TestKeyResponse {
            valid: false,
            message: "Unknown provider. Use `nebius`, `nim`, `tavily`, or `github`.".to_string(),
            model_tested: String::new(),
            provider: String::new(),
        });
    };
    let provider = kind.display_name().to_string();
    let model = kind.model();
    let respond = |valid: bool, message: String| {
        Json(TestKeyResponse {
            valid,
            message,
            model_tested: model.to_string(),
            provider: provider.clone(),
        })
    };

    let key = normalize_api_key(&req.api_key).or_else(|| state.llm.key_store().api_key(kind));
    let Some(key) = key else {
        return respond(
            false,
            format!(
                "No key to test. Paste one, or save it first (environment: {}).",
                kind.key_env_names().join(" or ")
            ),
        );
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
    {
        Ok(client) => client,
        Err(_) => return respond(false, "HTTP client initialization failed.".to_string()),
    };

    let endpoint = format!(
        "{}/chat/completions",
        kind.base_url(&state.config).trim_end_matches('/')
    );
    let result = client
        .post(endpoint)
        .bearer_auth(&key)
        .json(&serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": "ping" }],
            "max_tokens": 1,
            "temperature": 0.0
        }))
        .send()
        .await;

    match result {
        Ok(response) if response.status().is_success() => {
            respond(true, format!("Key works. Model `{model}` responded."))
        }
        Ok(response) => respond(false, describe_provider_status(response.status(), model)),
        Err(error) => respond(
            false,
            format!("Network request failed: {}", error.without_url()),
        ),
    }
}

/// Tests a Tavily or GitHub credential with one cheap request. Response bodies are never echoed.
async fn test_service_key(state: &AppState, service: &str, pasted: &str) -> Json<TestKeyResponse> {
    let (name, label) = if service == "tavily" {
        (KeyName::TavilyApiKey, "Tavily")
    } else {
        (KeyName::GithubToken, "GitHub")
    };
    let respond = |valid: bool, message: String| {
        Json(TestKeyResponse {
            valid,
            message,
            model_tested: String::new(),
            provider: label.to_string(),
        })
    };
    let Some(key) = normalize_api_key(pasted).or_else(|| state.keys.resolve(name)) else {
        return respond(
            false,
            format!("No {label} key to test. Paste one, or save it first."),
        );
    };
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("SetupPal")
        .build()
    {
        Ok(client) => client,
        Err(_) => return respond(false, "HTTP client initialization failed.".to_string()),
    };
    let request = if service == "tavily" {
        client
            .post(installer_core::safety::tavily::TAVILY_SEARCH_URL)
            .bearer_auth(&key)
            .json(&serde_json::json!({ "query": "ping", "max_results": 1 }))
    } else {
        client
            .get("https://api.github.com/rate_limit")
            .bearer_auth(&key)
    };
    match request.send().await {
        Ok(response) if response.status().is_success() => {
            respond(true, format!("{label} key works."))
        }
        Ok(response)
            if matches!(
                response.status(),
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
            ) =>
        {
            respond(
                false,
                format!("HTTP {}: {label} rejected this key.", response.status()),
            )
        }
        Ok(response) => respond(false, format!("HTTP {} from {label}.", response.status())),
        Err(error) => respond(
            false,
            format!("Network request failed: {}", error.without_url()),
        ),
    }
}

#[derive(Debug, Deserialize)]
struct SaveKeyRequest {
    name: String,
    value: String,
}

/// Stores a key in the OS keychain. The value is never echoed back or logged.
async fn save_key_handler(
    State(state): State<AppState>,
    Json(req): Json<SaveKeyRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let name = KeyName::parse(&req.name)
        .ok_or((StatusCode::BAD_REQUEST, "Unknown key name.".to_string()))?;
    let value = keys::validate_value(&req.value)
        .map_err(|reason| (StatusCode::BAD_REQUEST, reason.to_string()))?;
    state
        .keys
        .save(name, &value)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "name": name.as_str(), "saved": true }),
    ))
}

async fn delete_key_handler(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let name =
        KeyName::parse(&name).ok_or((StatusCode::BAD_REQUEST, "Unknown key name.".to_string()))?;
    state
        .keys
        .remove(name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "name": name.as_str(), "removed": true }),
    ))
}

/// Booleans only: never a key, a mask, or a prefix.
async fn key_status_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(state.keys.status())
}
fn describe_provider_status(status: StatusCode, model: &str) -> String {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            format!("HTTP {status}: the provider rejected this key.")
        }
        StatusCode::NOT_FOUND => {
            format!("HTTP {status}: model `{model}` is not available for this key.")
        }
        StatusCode::GONE => format!("HTTP {status}: model `{model}` has been retired."),
        StatusCode::TOO_MANY_REQUESTS => {
            format!("HTTP {status}: rate limited. Wait a moment and try again.")
        }
        _ => format!("HTTP {status} from the provider."),
    }
}

/// Reported in the context viewer so a reply can be traced to the prompt that produced it.
const PROMPT_VERSION: &str = "v28-backend-decides";

const STATIC_SYSTEM_PROMPT: &str = "\
Role\n\
You are Pixel, SetupPal's assistant: a friendly little ghost who helps people set up software on their Windows PC. Be warm and clear; at most one light joke per chat, never on errors or plans.\n\
\n\
Scope\n\
SetupPal installs anything that installs unattended and passes safety checks: winget, npm, pip, cargo, and CLI installers (Fabric for Minecraft Java). Game, music, chat, and dev apps are fine (LMMS, Discord, Steam, Roblox). A window or sign-in after install is fine.\n\
Out of scope: Microsoft Store-only apps (Minecraft Bedrock), vendor-page drivers. If unsure, emit an install intent; SetupPal checks.\n\
\n\
How to work\n\
- Answer the latest message using the whole conversation. Short replies like `php` continue the topic.\n\
- PC facts are included. Mention a fact only when it changes the answer. Never invent facts.\n\
- Broad request (\"I want to game\", \"I make music\"): ask what they play or need, offer 2 to 4 installable options in that category. No block yet.\n\
- Named request (\"vlc\", \"modded Minecraft Java with Fabric\") or agreement (\"yes\", \"do it\", \"set it up\"): one sentence on what you'll set up, then the intent block. Don't ask to confirm; the plan card does that. Don't list steps.\n\
- Never change a package name the user typed; ask if it looks misspelled.\n\
\n\
Manual setup guide\n\
Only when the user named a specific out-of-scope item: start with **Manual setup guide**, say why, short steps, then \"SetupPal can install instead:\" with 2 to 4 same-category options that are not already installed.\n\
\n\
Intent block\n\
Only when you are setting something up, end the reply with one fenced block (fence lines alone). Write the goal in the user's words. Chat, questions, and option lists get plain text and no block:\n\
```setuppal-intent\n\
{\"kind\":\"install\",\"goal\":\"vlc\"}\n\
```\n\
Out-of-scope named item: {\"kind\":\"manual\",\"item\":\"Minecraft Bedrock\",\"reason\":\"only sold in the Microsoft Store\"}.\n\
No block, no plan: if you say you'll set something up, include it. Skip it only while asking a question.\n\
\n\
Style\n\
Plain words, short replies. Don't open with a greeting after the first message. Don't mirror slang. Never claim you installed or checked anything.";

/// Plain-text facts about this PC. Minecraft details are only added when the chat is about Minecraft.
fn format_host_facts(
    profile: &installer_core::SystemProfile,
    mc_info: &MinecraftInspection,
    user_prompt: &str,
    history: &[ChatMessageItem],
) -> String {
    let gpu = profile
        .gpu_detected
        .as_deref()
        .or(mc_info.gpu_detected.as_deref())
        .unwrap_or("unknown");
    let disk_gb = profile.free_disk_bytes / (1024 * 1024 * 1024);

    let tools = profile
        .installed_tools
        .iter()
        .map(|t| {
            let ver = t
                .version_output
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .replace('"', "'");
            let ver = if ver.len() > 44 {
                format!("{}...", &ver[..ver.floor_char_boundary(44)])
            } else {
                ver
            };
            format!("{} ({})", t.binary.executable_name(), ver)
        })
        .collect::<Vec<_>>()
        .join(", ");

    let mut out = format!(
        "- OS: {} {} ({})\n- GPU: {}\n- Free disk: {} GB\n- CLI support: winget, user-scope PATH available\n- SetupPal can set up: anything with an unattended command-line install that a command can verify\n- Detected tools: {}",
        profile.os_name,
        profile.os_version,
        profile.arch,
        gpu,
        disk_gb,
        if tools.is_empty() { "none detected".to_string() } else { tools }
    );

    let recent = history
        .iter()
        .rev()
        .take(4)
        .map(|m| m.content.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    let blob = format!("{} {}", user_prompt.to_ascii_lowercase(), recent);
    let minecraft_requested = ["minecraft", "bedrock", "optifine"]
        .iter()
        .any(|k| blob.contains(k));

    if minecraft_requested {
        out.push_str("\n- Note: Minecraft Bedrock is Microsoft Store-only, so it needs a Manual setup guide. The Minecraft Launcher (winget), Java, and Fabric for Minecraft Java can be set up.");
    }

    out
}

/// Rough token estimate (about 4 characters per token).
fn est_tokens(s: &str) -> usize {
    s.chars().count().div_ceil(4)
}

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() > max {
        let t: String = s.chars().take(max).collect();
        format!("{}...", t.trim_end())
    } else {
        s.to_string()
    }
}

/// First meaningful line/sentence of a reply, used for compact notes.
fn gist(s: &str, max: usize) -> String {
    let line = s
        .lines()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let line = line.split(". ").next().unwrap_or(line);
    clip(line, max)
}

struct PromptContext {
    /// Compact notes covering older messages (never drops what the user asked for).
    memory_note: Option<String>,
    /// Newest messages, sent as written (clipped).
    recent: Vec<ChatMessageItem>,
    /// How many older messages were folded into the notes.
    compacted: usize,
    /// True once an assistant turn in this chat already told a joke (R20).
    joke_used: bool,
}

/// Heuristic: prior assistant text already contained a joke.
fn assistant_told_joke(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "why did the ghost",
        "why did the",
        "knock knock",
        "knock-knock",
        "lettuce",
        "who's there",
        "who is there",
        "ba dum",
        "here's a joke",
        "here is a joke",
        "joke:",
        "pun intended",
    ];
    MARKERS.iter().any(|m| t.contains(m))
}

const MEMORY_NOTE_CHARS: usize = 1800;

fn build_context(raw: &[ChatMessageItem], user_prompt: &str, keep_recent: usize) -> PromptContext {
    let keep = keep_recent.clamp(2, 16);
    let mut hist: Vec<&ChatMessageItem> = raw.iter().collect();
    if let Some(last) = hist.last() {
        if last.role == "user" && last.content.trim() == user_prompt.trim() {
            hist.pop();
        }
    }
    let split = hist.len().saturating_sub(keep);
    let (older, recent) = hist.split_at(split);

    // Notes are built newest-first so that, if space runs out, the oldest lines are the ones dropped.
    let mut lines: Vec<String> = Vec::new();
    let mut used = 0usize;
    let mut trimmed_any = false;
    for m in older.iter().rev() {
        let line = if m.role == "user" {
            format!("- User: {}", clip(&m.content, 220))
        } else {
            format!("- Assistant: {}", gist(&m.content, 140))
        };
        if used + line.len() > MEMORY_NOTE_CHARS {
            trimmed_any = true;
            break;
        }
        used += line.len() + 1;
        lines.push(line);
    }
    lines.reverse();
    let memory_note = if lines.is_empty() {
        None
    } else {
        let head = if trimmed_any {
            "(oldest notes dropped)\n"
        } else {
            ""
        };
        Some(format!("{}{}", head, lines.join("\n")))
    };

    let joke_used = older
        .iter()
        .chain(recent.iter())
        .filter(|m| m.role != "user")
        .any(|m| assistant_told_joke(&m.content));

    let recent: Vec<ChatMessageItem> = recent
        .iter()
        .map(|m| ChatMessageItem {
            role: if m.role == "user" {
                "user".into()
            } else {
                "assistant".into()
            },
            content: clip(&m.content, if m.role == "user" { 700 } else { 900 }),
        })
        .collect();

    PromptContext {
        memory_note,
        recent,
        compacted: older.len(),
        joke_used,
    }
}

type ChatSseSender = mpsc::Sender<Result<Event, Infallible>>;

async fn chat_stream_handler(
    State(state): State<AppState>,
    Json(mut req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let raw_msg = req.message.trim().to_string();
    if raw_msg.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Message cannot be empty.".to_string(),
        ));
    }

    let provider = state
        .llm
        .resolve()
        .map_err(|error| (StatusCode::SERVICE_UNAVAILABLE, error.to_string()))?;
    let profile = state
        .scanner
        .scan()
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let mc_info = inspect_minecraft_system(&profile);

    let history = req.history.take().unwrap_or_default();
    let context = build_context(&history, &raw_msg, req.memory_turns.unwrap_or(6));
    let host_facts = if req.include_pc_facts.unwrap_or(true) {
        format_host_facts(&profile, &mc_info, &raw_msg, &history)
    } else {
        "(PC facts are turned off by the user. Do not claim to know what is installed.)".to_string()
    };
    let max_tokens = req.max_reply_tokens.unwrap_or(800).clamp(200, 2000);
    let system_tokens = est_tokens(STATIC_SYSTEM_PROMPT);
    let memory_tokens = context.memory_note.as_deref().map(est_tokens).unwrap_or(0);
    let recent_tokens: usize = context
        .recent
        .iter()
        .map(|message| est_tokens(&message.content))
        .sum();
    let facts_tokens = est_tokens(&host_facts);
    let user_tokens = est_tokens(&raw_msg);
    let context_payload = serde_json::json!({
        "provider": provider.kind.id(),
        "model": provider.model,
        "prompt_version": PROMPT_VERSION,
        "system_tokens": system_tokens,
        "memory_tokens": memory_tokens,
        "recent_tokens": recent_tokens,
        "facts_tokens": facts_tokens,
        "user_tokens": user_tokens,
        "total_tokens": system_tokens + memory_tokens + recent_tokens + facts_tokens + user_tokens,
        "reply_budget": max_tokens,
        "total_messages": history.len(),
        "recent_messages": context.recent.len(),
        "compacted_messages": context.compacted,
        "memory_note": context.memory_note,
        "recent": context.recent,
        "facts": host_facts,
    });

    let os_provider = state.os_provider.clone();
    let installed_names: Vec<String> = profile
        .installed_tools
        .iter()
        .map(|t| t.binary.executable_name().to_ascii_lowercase())
        .collect();

    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(async move {
        if send_chat_event(
            &sender,
            "message.start",
            serde_json::json!({ "model": provider.model, "provider": provider.kind.id() }),
        )
        .await
        .is_err()
        {
            return;
        }

        let mut attempt = call_ai_chat_stream(&provider, &raw_msg, &context, &host_facts, max_tokens, &sender).await;
        if matches!(&attempt, Err(m) if m == EMPTY_REPLY_ERROR) {
            // Nothing visible was streamed for an empty reply, so one quiet retry is safe.
            attempt = call_ai_chat_stream(&provider, &raw_msg, &context, &host_facts, max_tokens, &sender).await;
        }
        match attempt {
            Ok((reply, intent)) => {
                let (reply, intent) = overrule_false_manual(os_provider.as_ref(), reply, intent).await;
                let intent = intent.map(|i| drop_installed_alternatives(i, &installed_names));
                if let Some(intent) = intent {
                    let _ = send_chat_event(
                        &sender,
                        "message.intent",
                        serde_json::json!({ "intent": intent }),
                    )
                    .await;
                }
                let _ = send_chat_event(
                    &sender,
                    "message.end",
                    serde_json::json!({ "reply": reply, "context": context_payload }),
                )
                .await;
            }
            Err(message) => {
                let _ = send_chat_event(
                    &sender,
                    "message.error",
                    serde_json::json!({ "message": message }),
                )
                .await;
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(receiver)).keep_alive(KeepAlive::default()))
}

async fn send_chat_event(
    sender: &ChatSseSender,
    event_name: &str,
    payload: serde_json::Value,
) -> Result<(), String> {
    let data = serde_json::to_string(&payload)
        .map_err(|_| "Could not encode stream event.".to_string())?;
    sender
        .send(Ok(Event::default().event(event_name).data(data)))
        .await
        .map_err(|_| "Chat stream was cancelled.".to_string())
}

fn build_chat_messages(
    user_prompt: &str,
    context: &PromptContext,
    host_facts: &str,
) -> Vec<serde_json::Value> {
    let mut messages: Vec<serde_json::Value> = vec![serde_json::json!({
        "role": "system",
        "content": STATIC_SYSTEM_PROMPT
    })];
    if context.joke_used {
        messages.push(serde_json::json!({
            "role": "system",
            "content": "joke_used: true. You already told a joke in this chat. Do not tell another."
        }));
    }
    if let Some(note) = &context.memory_note {
        messages.push(serde_json::json!({
            "role": "system",
            "content": format!("Earlier in this chat (compacted notes, oldest first). Treat what the user said here as still true:\n{note}")
        }));
    }
    for item in &context.recent {
        messages.push(serde_json::json!({ "role": item.role, "content": item.content }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": format!("PC facts (background for you only; never repeat or list them unless asked):\n{host_facts}\n\nUser message (answer this):\n{user_prompt}")
    }));
    messages
}

async fn call_ai_chat_stream(
    provider: &ResolvedProvider,
    user_prompt: &str,
    context: &PromptContext,
    host_facts: &str,
    max_tokens: u32,
    sender: &ChatSseSender,
) -> Result<(String, Option<SetupIntent>), String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|_| "Could not initialize the AI connection.".to_string())?;

    let messages = build_chat_messages(user_prompt, context, host_facts);
    let request_body = serde_json::json!({
        "model": provider.model,
        "messages": messages,
        "temperature": 0.5,
        "max_tokens": max_tokens,
        "stream": true
    });
    let response = {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let request = client
                .post(provider.chat_completions_url())
                .bearer_auth(provider.api_key())
                .json(&request_body)
                .send();
            let result = tokio::select! {
                _ = sender.closed() => return Err("Chat stream was cancelled.".to_string()),
                result = request => result,
            };
            match result {
                Ok(response) if response.status().is_success() => break response,
                Ok(response) => {
                    let status = response.status();
                    let retryable =
                        status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
                    if !retryable || attempt >= 3 {
                        return Err(format!(
                            "{}: {}",
                            provider.kind.display_name(),
                            describe_provider_status(status, provider.model)
                        ));
                    }
                }
                Err(error) if attempt >= 3 => {
                    return Err(format!(
                        "Could not connect to AI provider: {}",
                        error.without_url()
                    ));
                }
                Err(_) => {}
            }
            tokio::select! {
                _ = sender.closed() => return Err("Chat stream was cancelled.".to_string()),
                _ = tokio::time::sleep(std::time::Duration::from_millis(400 * attempt)) => {}
            }
        }
    };

    let mut body = response.bytes_stream();
    let mut pending = Vec::<u8>::new();
    let mut data_lines = Vec::<String>::new();
    let mut filter = IntentStreamFilter::new();
    let mut complete = false;
    loop {
        let next_chunk = tokio::select! {
            _ = sender.closed() => return Err("Chat stream was cancelled.".to_string()),
            next_chunk = body.next() => next_chunk,
        };
        let Some(chunk) = next_chunk else { break };
        let chunk = chunk.map_err(|error| format!("AI stream failed: {}", error.without_url()))?;
        pending.extend_from_slice(&chunk);
        while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = pending.drain(..=newline).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if consume_chat_sse_line(&line, &mut data_lines, &mut filter, sender).await? {
                complete = true;
                break;
            }
        }
        if complete {
            break;
        }
    }

    if !complete && !pending.is_empty() {
        consume_chat_sse_line(&pending, &mut data_lines, &mut filter, sender).await?;
    }
    if !complete && !data_lines.is_empty() {
        let final_line = [];
        consume_chat_sse_line(&final_line, &mut data_lines, &mut filter, sender).await?;
    }

    let (reply, intent_result) = filter.finish();
    let mut intent = match intent_result {
        Ok(intent) => intent,
        Err(kind) => {
            warn!(?kind, "setuppal-intent block present but failed validation");
            None
        }
    };

    let reply = strip_empty_fences(&reply);
    if !reply.chars().any(char::is_alphanumeric) {
        return Err(EMPTY_REPLY_ERROR.to_string());
    }
    // "coding tools" or "some apps" names no program, so it is not an install request.
    if matches!(&intent, Some(SetupIntent::Install { goal, items }) if items.is_empty() && is_vague_goal(goal)) {
        intent = None;
    }
    // kind:manual requires a specifically named out-of-scope item (R18).
    if matches!(&intent, Some(SetupIntent::Manual { item, .. }) if item.trim().is_empty() || is_broad_request(item) || is_vague_goal(item)) {
        intent = None;
    }
    // Mirrors the app's own check: the goal has to come from what the user said.
    if let Some(SetupIntent::Install { goal, .. }) = &intent {
        let said = user_said_blob(user_prompt, context);
        if !goal_in_user_text(goal, &said) {
            intent = None;
        }
    }
    // A reply that asks a question and promises nothing is not an install request.
    if intent.is_some() && reply.trim_end().ends_with('?') && !reply_promises_install(&reply) {
        intent = None;
    }

    if intent.is_none() && (reply_commits_to_setup(user_prompt, &reply) || reply_is_manual_guide(&reply)) {
        let _ = send_chat_event(sender, "plan.pending", serde_json::json!({})).await;
        intent = tokio::select! {
            _ = sender.closed() => return Err("Chat stream was cancelled.".to_string()),
            extracted = extract_install_intent(&client, provider, user_prompt, context, &reply) => extracted,
        };
    }
    Ok((reply, intent))
}

/// The model's "manual-only" call is a claim, not a verdict. When winget has one exact,
/// unattended-installable match for the item, the claim is wrong: turn it into an install
/// intent and say so, so the chat text and the plan card agree.
async fn overrule_false_manual(
    os: &dyn installer_core::OsProvider,
    reply: String,
    intent: Option<SetupIntent>,
) -> (String, Option<SetupIntent>) {
    let Some(SetupIntent::Manual { item, .. }) = &intent else {
        return (reply, intent);
    };
    let Some(id) = installer_core::resolver::resolve_winget_id(os, item).await else {
        return (reply, intent);
    };
    let plan = installer_core::resolver::single_winget_plan(item, &id);
    if first_manual(&check_plan(&plan, os).await).is_some() {
        return (reply, intent);
    }
    info!(item = %item, winget_id = %id, "model called an installable app manual-only; overruled");
    let goal = item.clone();
    (
        format!("I can set up {item} for you."),
        Some(SetupIntent::Install { goal, items: Vec::new() }),
    )
}

const EMPTY_REPLY_ERROR: &str = "AI provider returned no usable reply text. Please try again.";

/// "SetupPal can install instead" must never offer something already on the PC,
/// and must stay in the same category as the blocked item (R18/R22).
fn drop_installed_alternatives(intent: SetupIntent, installed: &[String]) -> SetupIntent {
    match intent {
        SetupIntent::Manual { item, reason, alternatives } => {
            // Broad / unnamed manual claims are dropped entirely — chat should ask instead.
            if item.trim().is_empty() || is_broad_request(&item) {
                return SetupIntent::Manual {
                    item,
                    reason,
                    alternatives: Vec::new(),
                };
            }
            let category = installer_core::detect_category(&item)
                .unwrap_or(installer_core::AppCategory::Utilities);
            let mut alternatives: Vec<String> = alternatives
                .into_iter()
                .filter(|alt| {
                    let lower = alt.to_ascii_lowercase();
                    !installed.iter().any(|tool| {
                        lower
                            .split(|c: char| !c.is_ascii_alphanumeric())
                            .any(|word| word == tool)
                    })
                })
                .collect();
            alternatives = installer_core::filter_same_category(&alternatives, category);
            if alternatives.len() < 2 {
                alternatives = installer_core::alternatives_for(category, installed, 4)
                    .into_iter()
                    .map(|a| a.name)
                    .collect();
            }
            SetupIntent::Manual { item, reason, alternatives }
        }
        other => other,
    }
}

const AGREEMENT_REPLIES: &[&str] = &[
    "yes", "yeah", "yep", "yup", "sure", "ok", "okay", "do it", "go", "go ahead", "set it up",
    "install it", "please", "yes please", "lets go", "let's go", "sounds good", "do that",
    "go for it", "proceed", "continue", "start", "lets do it", "let's do it",
];

const COMMIT_PHRASES: &[&str] = &[
    "i will set up", "i'll set up", "i will install", "i'll install", "i can set up",
    "i can install", "setting up", "installing", "let's set up", "let's get", "i'll get",
];

/// The user agreed, or the reply promised an install, yet no intent block arrived.
fn reply_commits_to_setup(user_prompt: &str, reply: &str) -> bool {
    if reply.trim_end().ends_with('?') {
        return false;
    }
    let said: String = user_prompt
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '\'')
        .collect();
    let said = said.split_whitespace().collect::<Vec<_>>().join(" ");
    if AGREEMENT_REPLIES.contains(&said.as_str()) {
        return true;
    }
    reply_promises_install(&reply)
}

fn user_said_blob(user_prompt: &str, context: &PromptContext) -> String {
    let mut blob = user_prompt.to_lowercase();
    for item in context.recent.iter().filter(|i| i.role == "user") {
        blob.push('\n');
        blob.push_str(&item.content.to_lowercase());
    }
    blob
}

fn goal_in_user_text(goal: &str, said: &str) -> bool {
    const FILLER: &[&str] = &[
        "a", "an", "and", "the", "for", "with", "to", "of", "on", "in", "my", "plus", "set", "setup", "up",
        "install", "installer", "latest", "version", "app", "mod", "mods", "modded", "loader",
    ];
    let g = goal.to_lowercase();
    let g = g.trim();
    if g.is_empty() {
        return false;
    }
    if said.contains(g) {
        return true;
    }
    if let Some(last) = g.rsplit('.').next() {
        if last.len() >= 2 && said.contains(last) {
            return true;
        }
    }
    let words: Vec<&str> = g
        .split(|c: char| !(c.is_ascii_alphanumeric() || "+#.".contains(c)))
        .filter(|w| !w.is_empty() && !FILLER.contains(w))
        .collect();
    !words.is_empty() && words.iter().all(|w| said.contains(w))
}

fn reply_is_manual_guide(reply: &str) -> bool {
    reply.trim_start().to_lowercase().starts_with("**manual setup guide**")
}

const VAGUE_GOAL_WORDS: &[&str] = &[
    "some", "any", "tools", "tool", "apps", "app", "software", "programs", "program", "things", "stuff",
    "coding", "code", "learning", "learn", "to", "start", "started", "beginner", "dev", "development", "developer", "programming", "gaming", "game", "games",
    "music", "setup", "essentials", "basics", "everything", "and", "the", "a", "for", "my", "pc", "computer",
];

/// True when a goal only names a category, never a program.
fn is_vague_goal(goal: &str) -> bool {
    goal.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .all(|w| VAGUE_GOAL_WORDS.contains(&w))
}

fn reply_promises_install(reply: &str) -> bool {
    let reply = reply.to_lowercase().replace('\u{2019}', "'");
    COMMIT_PHRASES.iter().any(|p| reply.contains(p))
}

/// Removes code fences with nothing inside (a leftover of a stripped block).
fn strip_empty_fences(reply: &str) -> String {
    let mut out = reply.to_string();
    while let Some(start) = out.find("```") {
        let after = &out[start + 3..];
        let Some(end) = after.find("```") else { break };
        if after[..end].trim().is_empty() {
            out.replace_range(start..start + 6 + end, "");
        } else {
            break;
        }
    }
    out.trim_end().to_string()
}

const INTENT_EXTRACT_PROMPT: &str = "\
You turn a chat into a SetupPal install request. Reply with one JSON object only, no other text.\n\
If the user asked for or agreed to install something specific: {\"kind\":\"install\",\"goal\":\"<what to install, in the user's words>\"}\n\
If the assistant said SetupPal cannot install a specific app and gave a manual guide: {\"kind\":\"manual\",\"item\":\"<app name>\",\"reason\":\"<short reason>\"}\n\
Otherwise: {\"kind\":\"none\"}";

/// One small non-streaming call that recovers the intent the chat reply forgot to include.
async fn extract_install_intent(
    client: &reqwest::Client,
    provider: &ResolvedProvider,
    user_prompt: &str,
    context: &PromptContext,
    reply: &str,
) -> Option<SetupIntent> {
    let mut transcript = String::new();
    for item in context.recent.iter().rev().take(6).rev() {
        let who = if item.role == "user" { "User" } else { "Assistant" };
        transcript.push_str(&format!("{who}: {}\n", clip(&item.content, 300)));
    }
    transcript.push_str(&format!("User: {}\nAssistant: {}", clip(user_prompt, 300), clip(reply, 300)));

    let body = serde_json::json!({
        "model": provider.model,
        "messages": [
            { "role": "system", "content": INTENT_EXTRACT_PROMPT },
            { "role": "user", "content": transcript },
        ],
        "temperature": 0,
        "max_tokens": 120,
        "stream": false
    });
    let response = client
        .post(provider.chat_completions_url())
        .bearer_auth(provider.api_key())
        .json(&body)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        warn!(status = %response.status(), "intent extraction call failed");
        return None;
    }
    let value: serde_json::Value = response.json().await.ok()?;
    let content = value["choices"][0]["message"]["content"].as_str()?;
    parse_extracted_intent(content)
}

fn parse_extracted_intent(content: &str) -> Option<SetupIntent> {
    let content = content.rsplit("</think>").next().unwrap_or(content);
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    if end <= start {
        return None;
    }
    let json = &content[start..=end];
    let kind = serde_json::from_str::<serde_json::Value>(json).ok()?;
    if kind["kind"] != "install" && kind["kind"] != "manual" {
        return None;
    }
    match intent::parse_intent_json(json) {
        Ok(intent) => Some(intent),
        Err(kind) => {
            warn!(?kind, "extracted intent failed validation");
            None
        }
    }
}

async fn consume_chat_sse_line(
    line: &[u8],
    data_lines: &mut Vec<String>,
    filter: &mut IntentStreamFilter,
    sender: &ChatSseSender,
) -> Result<bool, String> {
    if line.is_empty() {
        if data_lines.is_empty() {
            return Ok(false);
        }
        let payload = data_lines.join("\n");
        data_lines.clear();
        if payload.trim() == "[DONE]" {
            return Ok(true);
        }
        let value: serde_json::Value = serde_json::from_str(&payload)
            .map_err(|_| "AI provider sent an invalid streaming response.".to_string())?;
        if let Some(text) = value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("delta"))
            .and_then(|delta| delta.get("content"))
            .and_then(serde_json::Value::as_str)
            .filter(|text| !text.is_empty())
        {
            let forward = filter.push(text);
            if !forward.is_empty() {
                send_chat_event(
                    sender,
                    "message.delta",
                    serde_json::json!({ "text": forward }),
                )
                .await?;
            }
            if filter.take_intent_opened() {
                send_chat_event(sender, "plan.pending", serde_json::json!({})).await?;
            }
        }
        return Ok(false);
    }

    if line.starts_with(b":") {
        return Ok(false);
    }
    if let Some(data) = line.strip_prefix(b"data:") {
        data_lines.push(String::from_utf8_lossy(data).trim().to_string());
    }
    Ok(false)
}

#[derive(Debug, Deserialize)]
struct PlanRequest {
    goal: String,
    untrusted_readme: Option<String>,
    /// Compact conversation summary for the LLM planner only (max 1,500 chars).
    context: Option<String>,
    /// The model called `goal` manual-only. Skip recipes and the LLM: plan it only if winget has
    /// one exact match, otherwise answer `{"kind":"no_match"}`.
    #[serde(default)]
    verify_manual: bool,
}

fn uniquify_plan_id(plan: &mut Plan) {
    let suffix = &Uuid::new_v4().to_string()[..8];
    if plan.plan_id.starts_with("plan-recipe-") {
        plan.plan_id = format!("{}-{}", plan.plan_id, suffix);
    } else if !plan.plan_id.contains('-') || plan.plan_id.len() < 12 {
        plan.plan_id = format!("plan-{}-{}", plan.plan_id, suffix);
    } else {
        plan.plan_id = format!("{}-{}", plan.plan_id, suffix);
    }
}

async fn plan_handler(
    State(state): State<AppState>,
    Json(req): Json<PlanRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    info!("Generating plan for goal: {:?}", req.goal);
    let profile = state
        .scanner
        .scan()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let context = req
        .context
        .as_deref()
        .map(|c| {
            let t = c.trim();
            if t.chars().count() > 1500 {
                t.chars().take(1500).collect::<String>()
            } else {
                t.to_string()
            }
        })
        .filter(|c| !c.is_empty());

    // Broad / category-only goals are chat questions, never a manual card (R18).
    if req.verify_manual && is_broad_request(&req.goal) {
        return Ok(Json(serde_json::json!({
            "kind": "broad",
            "message": "Ask what they need and offer 2 to 4 installable options in that category.",
        })));
    }

    let mut plan = if req.verify_manual {
        let goal = req.goal.trim();
        match installer_core::resolver::resolve_winget_id(state.os_provider.as_ref(), goal).await {
            Some(id) => installer_core::resolver::single_winget_plan(goal, &id),
            None => return Ok(Json(serde_json::json!({ "kind": "no_match" }))),
        }
    } else {
        match state
            .planner
            .create_plan_with_context(
                &req.goal,
                &profile,
                req.untrusted_readme.as_deref(),
                context.as_deref(),
            )
            .await
        {
            Ok(plan) => plan,
            Err(e) => {
                let msg = e.to_string();
                if let Some(payload) = parse_confirm_validation(&msg) {
                    return Ok(Json(payload));
                }
                return Err((StatusCode::INTERNAL_SERVER_ERROR, msg));
            }
        }
    };
    uniquify_plan_id(&mut plan);

    let install_checks = check_plan(&plan, state.os_provider.as_ref()).await;
    if let Some(manual) = first_manual(&install_checks) {
        let installer_core::Installability::ManualOnly { reason, detail } = &manual.installability
        else {
            unreachable!("first_manual only returns ManualOnly");
        };
        if is_broad_request(&manual.item) {
            return Ok(Json(serde_json::json!({
                "kind": "broad",
                "message": "Ask what they need and offer 2 to 4 installable options in that category.",
            })));
        }
        let installed: Vec<String> = profile
            .installed_tools
            .iter()
            .map(|t| t.binary.executable_name().to_ascii_lowercase())
            .collect();
        let guide = manual_guide_with_installed(&manual.item, *reason, detail, &installed);
        return Ok(Json(serde_json::json!(guide)));
    }

    let mut safety_report = state
        .safety_engine
        .evaluate_plan(&plan)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    apply_to_safety_report(&install_checks, &mut safety_report);
    enrich_plan_versions(&mut plan, &state).await;
    let step_metadata = create_metadata_for_plan(&plan, &safety_report);

    state
        .plans
        .write()
        .await
        .insert(plan.plan_id.clone(), (plan.clone(), safety_report.clone()));

    Ok(Json(serde_json::json!({
        "kind": "plan",
        "plan": plan,
        "safety": safety_report,
        "profile": profile,
        "step_metadata": step_metadata,
    })))
}

#[derive(Debug, Deserialize)]
struct ExecuteRequest {
    run_id: Option<String>,
    plan_id: Option<String>,
    dry_run: Option<bool>,
    auto_approve_green: Option<bool>,
    approved_step_ids: Option<Vec<String>>,
    allow_yellow_steps: Option<Vec<String>>,
    start_at_step_id: Option<String>,
    project_dir: Option<String>,
    trusted_categories: Option<Vec<TrustCategory>>,
    safety_level: Option<String>,
    /// Ignored when `plan_id` is already stored server-side.
    custom_plan: Option<Plan>,
}

fn resolve_auto_approve_green(req: &ExecuteRequest) -> bool {
    if let Some(explicit) = req.auto_approve_green {
        return explicit;
    }
    match req.safety_level.as_deref() {
        Some("strict") => false,
        Some("relaxed") | Some("balanced") => true,
        _ => false,
    }
}

/// Folder every relative step path resolves against. Never uses the backend's own working
/// directory (that created junk folders inside the app). Prefer a user-picked project folder,
/// otherwise `%USERPROFILE%\.setuppal\work`.
fn resolve_working_dir(plan: &Plan, project_dir: Option<&str>) -> Result<PathBuf, String> {
    let needs_project = plan.steps.iter().any(|s| {
        matches!(
            s.step,
            installer_core::Step::InstallPackage {
                ecosystem: installer_core::Ecosystem::Npm,
                ..
            }
        )
    });
    let requested = project_dir.map(str::trim).filter(|p| !p.is_empty());
    match requested {
        Some(dir) => {
            let profile = installer_core::paths::user_profile_dir()
                .ok_or_else(|| "Could not locate your user profile folder.".to_string())?;
            installer_core::paths::validate_project_dir(std::path::Path::new(dir), &profile)
                .map_err(|e| e.to_string())
        }
        None if needs_project => {
            Err("This plan installs npm packages. Choose a project folder first.".to_string())
        }
        None => {
            let profile = installer_core::paths::user_profile_dir()
                .ok_or_else(|| "Could not locate your user profile folder.".to_string())?;
            let work = profile.join(".setuppal").join("work");
            std::fs::create_dir_all(&work)
                .map_err(|e| format!("Could not create work folder: {e}"))?;
            Ok(work)
        }
    }
}
async fn execute_handler(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if let Some(level) = req.safety_level.as_deref() {
        let normalized = match level {
            "strict" | "balanced" | "relaxed" => level,
            _ => "strict",
        };
        *state.safety_level.write().await = normalized.to_string();
    }

    let plan_id = req.plan_id.clone().filter(|id| !id.trim().is_empty());
    let (plan, _cached_safety) = if let Some(id) = &plan_id {
        let guard = state.plans.read().await;
        match guard.get(id) {
            Some((p, s)) => (p.clone(), s.clone()),
            None => {
                return Err((StatusCode::NOT_FOUND, format!("Unknown plan_id `{id}`.")));
            }
        }
    } else if let Some(p) = req.custom_plan.clone() {
        // Legacy fallback only when the server never issued a plan_id.
        let s = state
            .safety_engine
            .evaluate_plan(&p)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        state
            .plans
            .write()
            .await
            .insert(p.plan_id.clone(), (p.clone(), s.clone()));
        (p, s)
    } else {
        return Err((StatusCode::BAD_REQUEST, "plan_id is required.".to_string()));
    };

    let working_dir = resolve_working_dir(&plan, req.project_dir.as_deref())
        .map_err(|message| (StatusCode::BAD_REQUEST, message))?;

    // Fresh safety verdicts right before execution.
    let mut safety_report = state
        .safety_engine
        .evaluate_plan(&plan)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let install_checks = check_plan(&plan, state.os_provider.as_ref()).await;
    apply_to_safety_report(&install_checks, &mut safety_report);
    state
        .plans
        .write()
        .await
        .insert(plan.plan_id.clone(), (plan.clone(), safety_report.clone()));

    let run_id = req
        .run_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let cancel = CancellationToken::new();
    {
        let mut runs = state.runs.lock().await;
        runs.insert(run_id.clone(), cancel.clone());
    }

    // Serialize installs/uninstalls so retries and Rewind never pile up behind a stuck winget.
    let _install_guard = state.install_lock.try_lock().map_err(|_| {
        (
            StatusCode::CONFLICT,
            "Another install or rewind is already running. Wait for it to finish, then try again."
                .to_string(),
        )
    })?;

    info!("Executing plan '{}': {}", plan.plan_id, plan.goal);

    let options = ExecutionOptions {
        dry_run: req.dry_run.unwrap_or(false),
        auto_approve_green: resolve_auto_approve_green(&req),
        approved_step_ids: req
            .approved_step_ids
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect(),
        allow_yellow_steps: req
            .allow_yellow_steps
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect(),
        start_at_step_id: req.start_at_step_id.clone(),
        working_dir,
        trusted_categories: req
            .trusted_categories
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect(),
        cancel: cancel.clone(),
        run_id: run_id.clone(),
    };

    let verifier = installer_core::Verifier::new(state.os_provider.clone(), state.events.clone());
    let adapters = installer_core::AdapterRegistry::with_github_token(
        state.os_provider.clone(),
        state.keys.provider(KeyName::GithubToken),
    );
    let executor = installer_core::Executor::new(
        adapters,
        state.os_provider.clone(),
        state.state_logger.clone(),
        verifier,
        state.events.clone(),
    );

    let summary = match executor.execute_plan(&plan, &safety_report, &options).await {
        Ok(summary) => summary,
        Err(e) => {
            state.runs.lock().await.remove(&run_id);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        }
    };

    let mut remediation = None;
    let failed = summary
        .step_results
        .iter()
        .find(|r| r.status == installer_core::StepRunStatus::Failed);
    if let Some(failed_step) = failed {
        if !summary.cancelled && !cancel.is_cancelled() {
            let profile = match state.scanner.scan().await {
                Ok(p) => p,
                Err(_) => SystemProfile {
                    os_name: state.os_provider.os_name(),
                    os_version: String::new(),
                    arch: state.os_provider.arch(),
                    free_disk_bytes: 0,
                    gpu_detected: None,
                    installed_tools: vec![],
                    active_path_entries: vec![],
                },
            };
            let successful: Vec<_> = plan
                .steps
                .iter()
                .take_while(|s| s.id != failed_step.step_id)
                .cloned()
                .collect();
            let failure = FailureContext::new(
                failed_step.step_id.clone(),
                failed_step.verification.as_ref().and_then(|v| v.exit_code),
                failed_step
                    .verification
                    .as_ref()
                    .map(|v| v.stdout.clone())
                    .unwrap_or_default(),
                failed_step
                    .error
                    .clone()
                    .or_else(|| failed_step.verification.as_ref().map(|v| v.stderr.clone()))
                    .unwrap_or_default(),
                profile,
                successful,
                1,
            );
            let llm: Arc<dyn LlmClient> = state.llm.clone();
            let healer = Healer::new(llm, state.safety_engine.clone(), state.events.clone())
                .with_max_attempts(1);
            if let Ok(Some((mut rem_plan, rem_safety))) =
                healer.diagnose_and_propose(&plan.goal, &failure).await
            {
                uniquify_plan_id(&mut rem_plan);
                state.plans.write().await.insert(
                    rem_plan.plan_id.clone(),
                    (rem_plan.clone(), rem_safety.clone()),
                );
                let has_yellow = rem_safety
                    .step_reports
                    .iter()
                    .any(|r| r.rating == RiskRating::Yellow);
                let is_red = rem_safety.overall_rating == RiskRating::Red;
                let meta = create_metadata_for_plan(&rem_plan, &rem_safety);
                if is_red {
                    remediation = Some(serde_json::json!({
                        "kind": "plan",
                        "plan": rem_plan,
                        "safety": rem_safety,
                        "step_metadata": meta,
                        "blocked": true,
                    }));
                } else if has_yellow || !options.auto_approve_green {
                    remediation = Some(serde_json::json!({
                        "kind": "plan",
                        "plan": rem_plan,
                        "safety": rem_safety,
                        "step_metadata": meta,
                    }));
                } else {
                    let rem_options = ExecutionOptions {
                        approved_step_ids: rem_plan.steps.iter().map(|s| s.id.clone()).collect(),
                        allow_yellow_steps: std::collections::HashSet::new(),
                        start_at_step_id: None,
                        run_id: format!("{run_id}-heal"),
                        cancel: cancel.clone(),
                        ..options.clone()
                    };
                    match executor
                        .execute_plan(&rem_plan, &rem_safety, &rem_options)
                        .await
                    {
                        Ok(rem_summary) => {
                            remediation = Some(serde_json::json!({
                                "kind": "plan",
                                "plan": rem_plan,
                                "safety": rem_safety,
                                "step_metadata": meta,
                                "summary": rem_summary,
                                "auto_ran": true,
                            }));
                        }
                        Err(e) => {
                            warn!("Remediation execute failed: {e}");
                            remediation = Some(serde_json::json!({
                                "kind": "plan",
                                "plan": rem_plan,
                                "safety": rem_safety,
                                "step_metadata": meta,
                            }));
                        }
                    }
                }
            }
        }
    }

    state.runs.lock().await.remove(&run_id);

    Ok(Json(serde_json::json!({
        "plan": plan,
        "safety": safety_report,
        "summary": summary,
        "dry_run": summary.dry_run,
        "mode": if summary.dry_run { "dry_run" } else { "live" },
        "headline": summary.headline(),
        "remediation": remediation,
        "run_id": run_id,
    })))
}

#[derive(Debug, Deserialize)]
struct CancelExecuteRequest {
    run_id: String,
}

async fn cancel_execute_handler(
    State(state): State<AppState>,
    Json(req): Json<CancelExecuteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let runs = state.runs.lock().await;
    match runs.get(&req.run_id) {
        Some(token) => {
            token.cancel();
            Ok(Json(serde_json::json!({ "status": "cancelling" })))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            "Unknown or finished run_id.".to_string(),
        )),
    }
}
async fn history_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let transactions = state
        .state_logger
        .read_all_transactions()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "count": transactions.len(),
        "transactions": transactions,
    })))
}

#[derive(Debug, Deserialize)]
struct HistoryExportQuery {
    format: Option<String>,
}

async fn history_export_handler(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<HistoryExportQuery>,
) -> Result<(axum::http::HeaderMap, String), (StatusCode, String)> {
    let fmt = query.format.as_deref().unwrap_or("html");
    let mut headers = axum::http::HeaderMap::new();

    if fmt == "md" || fmt == "markdown" {
        let md = state
            .state_logger
            .export_audit_report_markdown()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        headers.insert(
            header::CONTENT_TYPE,
            "text/markdown; charset=utf-8".parse().unwrap(),
        );
        Ok((headers, md))
    } else {
        let html = state
            .state_logger
            .export_audit_report_html()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        headers.insert(
            header::CONTENT_TYPE,
            "text/html; charset=utf-8".parse().unwrap(),
        );
        Ok((headers, html))
    }
}

#[derive(Debug, Deserialize)]
struct UndoRequest {
    tx_id: Option<String>,
}

async fn undo_handler(
    State(state): State<AppState>,
    Json(req): Json<UndoRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let _install_guard = state.install_lock.try_lock().map_err(|_| {
        (
            StatusCode::CONFLICT,
            "Another install or rewind is already running. Wait for it to finish, then try again."
                .to_string(),
        )
    })?;

    let tx = match req.tx_id.as_deref() {
        Some(id) => state
            .state_logger
            .get_transaction_by_id(id)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
        None => state
            .state_logger
            .get_latest_transaction()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
    };

    match tx {
        Some(t) => {
            // Idempotent: never start a second uninstall for the same change.
            if state
                .state_logger
                .is_transaction_rolled_back(&t.tx_id)
                .unwrap_or(false)
            {
                return Ok(Json(serde_json::json!({
                    "status": "rolled_back",
                    "tx_id": t.tx_id,
                    "outcome": "already_undone",
                })));
            }
            let outcome = state
                .state_logger
                .rollback_transaction(&t, &state.os_provider)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            Ok(Json(serde_json::json!({
                "status": "rolled_back",
                "tx_id": t.tx_id,
                "outcome": outcome,
            })))
        }
        None => Ok(Json(serde_json::json!({
            "status": "error",
            "message": "No matching transaction found in state log to rollback."
        }))),
    }
}

async fn events_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(agent_event) => match serde_json::to_string(&agent_event) {
            Ok(json) => Some(Ok(Event::default().data(json))),
            Err(e) => {
                error!("Failed to serialize AgentEvent: {}", e);
                None
            }
        },
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(test)]
mod provider_selection_tests {
    use super::*;
    use installer_core::llm::KeyStore;

    struct FixedKeys {
        nebius: bool,
        nim: bool,
    }

    impl KeyStore for FixedKeys {
        fn api_key(&self, kind: ProviderKind) -> Option<String> {
            let present = match kind {
                ProviderKind::Nebius => self.nebius,
                ProviderKind::Nim => self.nim,
            };
            present.then(|| format!("test-{}-secret", kind.id()))
        }
    }

    fn client(provider: Option<&str>, nebius: bool, nim: bool) -> ProviderClient {
        let config = CoreConfig {
            provider: provider.map(str::to_string),
            nim_base_url: CoreConfig::DEFAULT_NIM_BASE_URL.to_string(),
            nim_api_key: None,
            nebius_base_url: CoreConfig::DEFAULT_NEBIUS_BASE_URL.to_string(),
            nebius_api_key: None,
            tavily_api_key: None,
            recipes_dir: PathBuf::from("recipes"),
            state_log_path: PathBuf::from(".orchestrator_state.jsonl"),
        };
        ProviderClient::with_key_store(config, Arc::new(FixedKeys { nebius, nim }))
            .expect("client builds")
    }

    #[test]
    fn explicit_nim_is_used_by_health_chat_and_planner_with_both_keys() {
        use installer_core::LlmClient;
        let llm = client(Some("nim"), true, true);
        let status = provider_status(&llm);
        assert_eq!(status["provider"], "nim");
        assert_eq!(status["model"], CoreConfig::NIM_MODEL);
        assert_eq!(status["provider_ready"], true);
        assert_eq!(
            llm.resolve().expect("resolves").model,
            CoreConfig::NIM_MODEL
        );
        assert_eq!(llm.model_name(), CoreConfig::NIM_MODEL);
    }

    #[test]
    fn unset_provider_defaults_to_nebius_when_its_key_exists() {
        let status = provider_status(&client(None, true, true));
        assert_eq!(status["provider"], "nebius");
        assert_eq!(status["model"], CoreConfig::NEBIUS_MODEL);
    }

    #[test]
    fn unset_provider_falls_back_to_nim_without_a_nebius_key() {
        let status = provider_status(&client(None, false, true));
        assert_eq!(status["provider"], "nim");
        assert_eq!(status["has_nebius_key"], false);
        assert_eq!(status["has_nim_key"], true);
    }

    #[test]
    fn missing_key_is_reported_without_leaking_values() {
        let status = provider_status(&client(Some("nebius"), false, true));
        assert_eq!(status["provider"], "nebius");
        assert_eq!(status["provider_ready"], false);
        let text = status.to_string();
        assert!(text.contains("NEBIUS_API_KEY"));
        assert!(!text.contains("secret"));
    }

    #[test]
    fn invalid_provider_value_is_reported() {
        let status = provider_status(&client(Some("openai"), true, true));
        assert!(status["provider"].is_null());
        assert_eq!(status["provider_ready"], false);
    }
}

#[cfg(test)]
mod prompt_tests {
    use super::*;

    fn message(role: &str, content: &str) -> ChatMessageItem {
        ChatMessageItem {
            role: role.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn system_prompt_is_pixel_only_and_short() {
        assert!(STATIC_SYSTEM_PROMPT.contains("You are Pixel"));
        assert!(!STATIC_SYSTEM_PROMPT.contains("Byte"));
        assert!(!STATIC_SYSTEM_PROMPT.contains("Mochi"));
        assert!(STATIC_SYSTEM_PROMPT.contains("Manual setup guide"));
        assert!(STATIC_SYSTEM_PROMPT.contains("setuppal-intent"));
        assert!(STATIC_SYSTEM_PROMPT.split_whitespace().count() <= 350);
    }

    #[test]
    fn agreement_or_a_promised_install_triggers_intent_recovery() {
        assert!(reply_commits_to_setup("do it", "I will set up Fabric for Minecraft Java."));
        assert!(reply_commits_to_setup("Set it up!", "On it."));
        assert!(reply_commits_to_setup("fabric", "I'll set up Fabric for Minecraft Java."));
        assert!(!reply_commits_to_setup("do it", "Which version of Python do you want?"));
        assert!(!reply_commits_to_setup("what is winget", "Winget is Windows' package manager."));
    }

    #[test]
    fn category_only_goals_are_vague_but_named_programs_are_not() {
        assert!(is_vague_goal("coding tools"));
        assert!(is_vague_goal("some gaming apps"));
        assert!(!is_vague_goal("vlc"));
        assert!(!is_vague_goal("Node.js"));
        assert!(!is_vague_goal("modded Minecraft Java with Fabric"));
        assert!(reply_is_manual_guide("  **Manual setup guide** Audacity..."));
    }

    #[test]
    fn empty_fences_are_removed_but_real_code_stays() {
        assert_eq!(strip_empty_fences("Options: VS Code.\n``` ```"), "Options: VS Code.");
        assert_eq!(strip_empty_fences("Run:\n```\nwinget list\n```"), "Run:\n```\nwinget list\n```");
        assert!(reply_promises_install("I\u{2019}ll set up VLC"));
    }

    #[test]
    fn manual_alternatives_skip_what_is_already_installed() {
        let intent = SetupIntent::Manual {
            item: "Minecraft Bedrock".into(),
            reason: "Store only".into(),
            alternatives: vec!["Git".into(), "Python 3.12".into(), "Steam".into(), "Prism Launcher".into()],
        };
        let got = drop_installed_alternatives(intent, &["git".to_string(), "python".to_string(), "steam".to_string()]);
        match got {
            SetupIntent::Manual { alternatives, .. } => {
                assert!(!alternatives.iter().any(|a| a == "Git" || a.contains("Python") || a == "Steam"));
                assert!(alternatives.len() >= 2 && alternatives.len() <= 4);
                // Same category as Minecraft Bedrock (gaming), not dev tools.
                assert!(alternatives.iter().any(|a| {
                    a.contains("Discord") || a.contains("Epic") || a.contains("OBS") || a.contains("Roblox") || a.contains("Afterburner")
                }));
            }
            _ => panic!("expected manual"),
        }
    }

    #[test]
    fn extracted_intent_accepts_install_only() {
        let got = parse_extracted_intent(
            "<think>hm</think>\n```json\n{\"kind\":\"install\",\"goal\":\"Fabric for Minecraft Java\"}\n```",
        );
        assert!(matches!(got, Some(SetupIntent::Install { ref goal, .. }) if goal == "Fabric for Minecraft Java"));
        assert!(parse_extracted_intent("{\"kind\":\"none\"}").is_none());
        assert!(parse_extracted_intent("no json here").is_none());
    }

    #[test]
    fn compacted_notes_label_assistant_turns_neutrally() {
        let history: Vec<ChatMessageItem> = (0..10)
            .flat_map(|i| {
                [
                    message("user", &format!("question {i}")),
                    message("assistant", &format!("answer {i}")),
                ]
            })
            .collect();
        let context = build_context(&history, "latest", 2);
        let note = context.memory_note.expect("older turns are compacted");
        assert!(note.contains("- User: question 0"));
        assert!(note.contains("- Assistant: answer 0"));
        assert!(!note.contains("- Pixel:"));
        assert_eq!(context.recent.len(), 2);
        assert!(!context.joke_used);
    }

    #[test]
    fn joke_used_is_detected_and_injected_into_chat_messages() {
        let history = vec![
            message("user", "hi"),
            message("assistant", "Why did the ghost cross the road? To get to the other side!"),
            message("user", "install vlc"),
        ];
        let context = build_context(&history, "install vlc", 6);
        assert!(context.joke_used);
        let messages = build_chat_messages("install vlc", &context, "- OS: test");
        assert!(messages.iter().any(|m| {
            m["role"] == "system"
                && m["content"]
                    .as_str()
                    .unwrap_or("")
                    .contains("joke_used: true")
        }));
    }

    #[test]
    fn confirm_validation_parses_into_payload() {
        let payload = parse_confirm_validation("plan validation error: confirm:PyPI:reqeusts:requests")
            .expect("payload");
        assert_eq!(payload["kind"], "confirm");
        assert_eq!(payload["typed"], "reqeusts");
        assert_eq!(payload["suggestion"], "requests");
        assert_eq!(payload["ecosystem"], "pypi");
    }

    #[test]
    fn chat_messages_start_with_the_static_prompt_and_end_with_facts_and_message() {
        let context = build_context(&[], "hi", 6);
        let messages = build_chat_messages("hi", &context, "- OS: test");
        assert_eq!(messages[0]["content"], STATIC_SYSTEM_PROMPT);
        let last = messages.last().expect("user turn");
        assert_eq!(last["role"], "user");
        assert!(last["content"]
            .as_str()
            .unwrap_or_default()
            .ends_with("User message (answer this):\nhi"));
    }
}

#[cfg(test)]
mod project_dir_tests {
    use super::*;
    use installer_core::{AdapterKind, Ecosystem, PlanSource, PlannedStep, Step};

    fn plan_with(ecosystem: Ecosystem, adapter: AdapterKind) -> Plan {
        Plan {
            plan_id: "p".to_string(),
            goal: "g".to_string(),
            summary: "s".to_string(),
            source: PlanSource::Llm {
                model: "m".to_string(),
            },
            steps: vec![PlannedStep {
                id: "step-1".to_string(),
                description: "install".to_string(),
                step: Step::InstallPackage {
                    ecosystem,
                    package_id: "typescript".to_string(),
                    version: None,
                    source_adapter: adapter,
                },
            }],
        }
    }

    #[test]
    fn npm_plans_require_a_project_folder() {
        let plan = plan_with(Ecosystem::Npm, AdapterKind::Npm);
        let err = resolve_working_dir(&plan, None).expect_err("needs a folder");
        assert!(err.contains("Choose a project folder"));
        assert!(resolve_working_dir(&plan, Some("   ")).is_err());
    }

    #[test]
    fn non_npm_plans_do_not_need_a_folder() {
        let plan = plan_with(Ecosystem::Winget, AdapterKind::Winget);
        assert!(resolve_working_dir(&plan, None).is_ok());
    }

    #[test]
    fn explicit_folders_must_exist_and_stay_inside_the_profile() {
        let plan = plan_with(Ecosystem::Npm, AdapterKind::Npm);
        let profile = installer_core::paths::user_profile_dir().expect("profile");
        let inside = tempfile::tempdir_in(&profile).expect("temp dir in profile");
        assert!(resolve_working_dir(&plan, inside.path().to_str()).is_ok());

        let missing = inside.path().join("missing");
        assert!(resolve_working_dir(&plan, missing.to_str()).is_err());

        let outside = if cfg!(windows) { "C:\\Windows" } else { "/" };
        let err = resolve_working_dir(&plan, Some(outside)).expect_err("outside the profile");
        assert!(err.contains("inside your user profile"));
    }

    #[test]
    fn an_invalid_folder_is_an_error_even_for_non_npm_plans() {
        let plan = plan_with(Ecosystem::Winget, AdapterKind::Winget);
        assert!(resolve_working_dir(&plan, Some("Z:\\definitely\\not\\here")).is_err());
    }
}

/// Live prompt corpus against the real provider. Run:
/// `cargo test -p installer_gui live_corpus -- --ignored --nocapture`
#[cfg(test)]
mod live_corpus {
    use super::*;

    enum Expect {
        Install(&'static str),
        Manual(&'static str),
        Nothing,
    }

    fn turn(role: &str, content: &str) -> ChatMessageItem {
        ChatMessageItem { role: role.into(), content: content.into() }
    }

    async fn ask(provider: &ResolvedProvider, history: &[ChatMessageItem], message: &str) -> (String, Option<SetupIntent>) {
        let context = build_context(history, message, 6);
        let (tx, mut rx) = mpsc::channel(256);
        tokio::spawn(async move { while rx.recv().await.is_some() {} });
        call_ai_chat_stream(provider, message, &context, "- OS: Windows 11 (x86_64)\n- Detected tools: git, node", 800, &tx)
            .await
            .expect("provider call")
    }

    fn has_numbered_steps(reply: &str) -> bool {
        reply.lines().filter(|l| {
            let t = l.trim_start();
            t.len() > 2 && t.as_bytes()[0].is_ascii_digit() && (t[1..].starts_with(". ") || t[1..].starts_with(") "))
        }).count() >= 2
    }

    #[tokio::test]
    #[ignore = "needs a provider key and network"]
    async fn model_follows_the_prompt() {
        let keys = Arc::new(Keys::with_dotenv(
            Arc::new(keys::KeyringStore),
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../.env")),
        ));
        let state = build_app_state(keys).expect("state");
        let provider = state.llm.resolve().expect("a provider key");
        println!("provider={} model={}", provider.kind.id(), provider.model);

        let fabric_hist = vec![
            turn("user", "Modded Minecraft Java with Fabric"),
            turn("assistant", "Modded Minecraft Java with Fabric sounds like fun!"),
        ];
        let cases: Vec<(&str, Vec<ChatMessageItem>, &str, Expect)> = vec![
            ("named app", vec![], "install vlc", Expect::Install("vlc")),
            ("named app 2", vec![], "I need audacity", Expect::Install("audacity")),
            ("pip", vec![], "pip install requests", Expect::Install("requests")),
            ("npm", vec![], "install typescript globally with npm", Expect::Install("typescript")),
            ("game launcher", vec![], "set up discord for me", Expect::Install("discord")),
            ("fabric first message", vec![], "Modded Minecraft Java with Fabric", Expect::Install("fabric")),
            ("fabric agree", fabric_hist.clone(), "set it up", Expect::Install("fabric")),
            ("fabric agree 2", fabric_hist, "do it", Expect::Install("fabric")),
            ("store only", vec![], "install Minecraft Bedrock", Expect::Manual("bedrock")),
            ("broad gaming", vec![], "I want to game on my new pc", Expect::Nothing),
            ("broad music", vec![], "I make music", Expect::Nothing),
            ("broad dev", vec![], "I'm learning to code", Expect::Nothing),
            ("smalltalk", vec![], "hi", Expect::Nothing),
            ("question", vec![], "what can you set up for me?", Expect::Nothing),
        ];

        let runs = 2;
        let mut failures = Vec::new();
        for (label, history, message, expect) in &cases {
            for run in 1..=runs {
                let (reply, intent) = ask(&provider, history, message).await;
                let (reply, intent) = overrule_false_manual(state.os_provider.as_ref(), reply, intent).await;
                let problem = match (expect, &intent) {
                    (Expect::Install(w), Some(SetupIntent::Install { goal, .. })) if goal.to_lowercase().contains(w) => None,
                    (Expect::Manual(w), Some(SetupIntent::Manual { item, .. })) if item.to_lowercase().contains(w) => None,
                    (Expect::Manual(w), Some(SetupIntent::Install { goal, .. })) if goal.to_lowercase().contains(w) => None,
                    (Expect::Nothing, None) => None,
                    (_, got) => Some(format!("intent was {got:?}")),
                }
                .or_else(|| has_numbered_steps(&reply).then(|| "reply lists steps".to_string()))
                .or_else(|| (reply.contains("setuppal-intent") || reply.contains("\"kind\"")).then(|| "intent leaked into the reply".to_string()));
                println!("[{label} #{run}] {} {:?} :: {}", if problem.is_none() { "ok" } else { "FAIL" }, problem, reply.replace('\n', " ").chars().take(110).collect::<String>());
                if let Some(p) = problem {
                    failures.push(format!("{label} #{run}: {p}"));
                }
            }
        }
        assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
    }

    #[tokio::test]
    #[ignore = "debug: raw reply for a greeting"]
    async fn raw_greeting() {
        let keys = Arc::new(Keys::with_dotenv(
            Arc::new(keys::KeyringStore),
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../.env")),
        ));
        let state = build_app_state(keys).expect("state");
        let provider = state.llm.resolve().expect("provider");
        let context = build_context(&[], "hi", 6);
        let body = serde_json::json!({
            "model": provider.model,
            "messages": build_chat_messages("hi", &context, "- OS: Windows 11"),
            "temperature": 0.5, "max_tokens": 800, "stream": false
        });
        let res = reqwest::Client::new().post(provider.chat_completions_url()).bearer_auth(provider.api_key()).json(&body).send().await.unwrap();
        println!("RAW {}", res.text().await.unwrap());
    }

    #[tokio::test]
    #[ignore = "needs winget and network; resolves and checks only, installs nothing"]
    async fn winget_resolver_and_gate() {
        let os: Arc<dyn installer_core::OsProvider> = Arc::new(installer_core::os::PlatformOsProvider::new());
        let ids = [
            ("audacity", "Audacity.Audacity"), ("ripgrep", "BurntSushi.ripgrep.MSVC"), ("jq", "jqlang.jq"),
            ("notepad++", "Notepad++.Notepad++"), ("lmms", "LMMS.LMMS"), ("discord", "Discord.Discord"),
            ("obs studio", "OBSProject.OBSStudio"), ("7-zip", "7zip.7zip"), ("vlc", "VideoLAN.VLC"),
            ("Audacity.Audacity", "Audacity.Audacity"),
        ];
        let mut bad = Vec::new();
        for (query, want) in ids {
            let got = installer_core::resolver::resolve_winget_id(os.as_ref(), query).await;
            println!("resolve {query:18} -> {got:?}");
            if got.as_deref() != Some(want) { bad.push(format!("{query}: wanted {want}, got {got:?}")); }
        }
        for app in ["LMMS", "Discord", "Spotify", "Steam", "Epic Games Launcher", "Roblox", "HWiNFO", "MSI Afterburner", "Notepad++"] {
            let Some(id) = installer_core::resolver::resolve_winget_id(os.as_ref(), app).await else {
                println!("gate {app:20} -> no unique winget match");
                bad.push(format!("{app}: no unique winget match"));
                continue;
            };
            let plan = installer_core::resolver::single_winget_plan(app, &id);
            let manual = first_manual(&check_plan(&plan, os.as_ref()).await).is_some();
            println!("gate {app:20} -> {id} manual={manual}");
            if manual { bad.push(format!("{app} ({id}) wrongly manual-only")); }
        }
        let bedrock = installer_core::resolver::resolve_winget_id(os.as_ref(), "Minecraft Bedrock").await;
        println!("gate Minecraft Bedrock    -> {bedrock:?}");
        if let Some(id) = bedrock {
            let plan = installer_core::resolver::single_winget_plan("Minecraft Bedrock", &id);
            if first_manual(&check_plan(&plan, os.as_ref()).await).is_none() {
                bad.push(format!("Minecraft Bedrock ({id}) should not be installable"));
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    #[tokio::test]
    #[ignore = "needs network and the saved Tavily key; evaluates plans, runs nothing"]
    async fn live_safety_on_real_plans() {
        use installer_core::{AdapterKind, Ecosystem, PlanSource, PlannedStep, RiskRating, Step};
        let keys = Arc::new(Keys::with_dotenv(
            Arc::new(keys::KeyringStore),
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../.env")),
        ));
        let state = build_app_state(keys).expect("state");
        let profile = state.scanner.scan().await.expect("scan");

        let plan = state.planner.create_plan("Modded Minecraft Java with Fabric", &profile, None).await.expect("recipe plan");
        let report = state.safety_engine.evaluate_plan(&plan).await.expect("report");
        for (s, r) in plan.steps.iter().zip(&report.step_reports) {
            println!("recipe  {:?} {:<42} {}", r.rating, s.description.chars().take(42).collect::<String>(), r.reasons.first().cloned().unwrap_or_default().chars().take(70).collect::<String>());
        }
        assert!(report.can_execute, "the Fabric recipe must be executable");
        assert!(report.step_reports.iter().all(|r| r.rating == RiskRating::Green), "recipe should be all green");

        let fake = |repo: &str| installer_core::Plan {
            plan_id: "hostile".into(),
            goal: repo.into(),
            summary: "x".into(),
            source: PlanSource::Llm { model: "m".into() },
            steps: vec![PlannedStep {
                id: "step-1".into(),
                description: format!("Install {repo}"),
                step: Step::InstallPackage {
                    ecosystem: Ecosystem::GitHub,
                    package_id: repo.into(),
                    version: None,
                    source_adapter: AdapterKind::GitHubReleases,
                },
            }],
        };
        let fake_report = state.safety_engine.evaluate_plan(&fake("obs-free-tools/obs-studio")).await.expect("report");
        println!("hostile {:?} {:?}", fake_report.overall_rating, fake_report.step_reports[0].reasons);
        assert_eq!(fake_report.overall_rating, RiskRating::Red, "a copycat repo must be blocked");
        assert!(!fake_report.can_execute);
    }
}