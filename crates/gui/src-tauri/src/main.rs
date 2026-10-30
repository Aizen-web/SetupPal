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
    apply_to_safety_report, check_plan, first_manual, manual_guide, AllowedBinary, CoreConfig,
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
    let planner = Arc::new(Planner::new(recipes.clone(), llm.clone(), events.clone()));
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
            serde_json::json!({
                "step_id": s.id,
                "source_domain": domain,
                "source_url": url,
                "verdict_title": verdict_title,
                "verdict_desc": verdict_desc,
            })
        })
        .collect()
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
const PROMPT_VERSION: &str = "v22-intent-block";

const STATIC_SYSTEM_PROMPT: &str = "\
Role\n\
You are Pixel, SetupPal's assistant: a friendly little ghost who helps people set up software on their Windows PC. Be playful, curious, and warm, with a light spooky joke now and then.\n\
\n\
Scope\n\
SetupPal can set up anything that installs unattended from the command line (silent winget, npm, pip, cargo, or CLI installers like Fabric for Minecraft Java), can be verified with a command such as `node -v`, and passes SetupPal's safety checks. That includes everyday apps, not just coding tools (for example Git, VS Code, 7-Zip, VLC, OBS Studio).\n\
Apps that need a GUI launcher, store login, or DRM (Steam, Epic Games, Riot, Battle.net, Roblox, Minecraft Bedrock, and similar) are out of scope.\n\
\n\
How to work\n\
- Answer the latest message using the whole conversation. Short replies like `php` continue the topic.\n\
- Each message includes PC facts. Use them; never invent facts. If something is installed, say its version.\n\
- Greeting or broad request: ask what they want set up, offer 2 to 4 options that fit this PC, recommend one.\n\
- Clear request: give the plan right away. Ask only when the answer changes the plan.\n\
- A plan is a short numbered list: what gets installed, its source, and the check command. Nothing installs until they approve.\n\
\n\
Manual setup guide\n\
If out of scope, start with **Manual setup guide**. Say SetupPal only does unattended terminal setup and why this needs a manual install. Give short numbered steps. End with \"SetupPal can install instead:\" and 2 to 4 options.\n\
\n\
Ending a reply\n\
When the user has agreed on what to install, end the reply with one fenced block, written exactly like this:\n\
```setuppal-intent\n\
{\"kind\":\"install\",\"goal\":\"typescript\"}\n\
```\n\
For an out-of-scope item use {\"kind\":\"manual\",\"item\":\"Steam\",\"reason\":\"needs a store login\"}. Never output the block while still asking a question.\n\
\n\
Style\n\
Plain words, short replies, markdown allowed, bold the key point. Don't introduce yourself unless asked. Never claim you installed or checked anything. Never send an empty reply.";

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
    let launcher_requested = [
        "minecraft",
        "bedrock",
        "steam",
        "riot",
        "epic",
        "launcher",
        "optifine",
        "roblox",
    ]
    .iter()
    .any(|k| blob.contains(k));

    if launcher_requested {
        out.push_str("\n- Note: the user mentioned software that may need a GUI launcher, store login, or DRM. If it does, reply with a Manual setup guide. Parts that install from the command line (for example Java or Fabric for Minecraft Java) can still be set up.");
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

    let recent = recent
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

        match call_ai_chat_stream(
            &provider,
            &raw_msg,
            &context,
            &host_facts,
            max_tokens,
            &sender,
        )
        .await
        {
            Ok((reply, intent)) => {
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
    let intent = match intent_result {
        Ok(intent) => intent,
        Err(kind) => {
            warn!(?kind, "setuppal-intent block present but failed validation");
            None
        }
    };

    if !reply.chars().any(char::is_alphanumeric) {
        return Err("AI provider returned no usable reply text. Please try again.".to_string());
    }
    Ok((reply, intent))
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

    let mut plan = state
        .planner
        .create_plan_with_context(
            &req.goal,
            &profile,
            req.untrusted_readme.as_deref(),
            context.as_deref(),
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    uniquify_plan_id(&mut plan);

    let install_checks = check_plan(&plan, state.os_provider.as_ref()).await;
    if let Some(manual) = first_manual(&install_checks) {
        let installer_core::Installability::ManualOnly { reason, detail } = &manual.installability
        else {
            unreachable!("first_manual only returns ManualOnly");
        };
        let guide = manual_guide(&manual.item, *reason, detail, &state.recipes);
        return Ok(Json(serde_json::json!(guide)));
    }

    let mut safety_report = state
        .safety_engine
        .evaluate_plan(&plan)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    apply_to_safety_report(&install_checks, &mut safety_report);
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

/// Folder every relative step path resolves against. Plans with npm installs need a folder the
/// user picked: it must exist and sit inside their profile. Other plans run in the backend's own
/// working directory, so an explicit but invalid folder is still an error, never a silent fallback.
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
        None => Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
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
