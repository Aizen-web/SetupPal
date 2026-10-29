//! SetupPal Zero-Trust Backend API & SSE Server
//!
//! Exposes installer_core domain logic to the modern UI frontend via HTTP & Server-Sent Events (SSE).

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, Method, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tokio_stream::{Stream, StreamExt};
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

use installer_core::llm::{ConfigKeyStore, KeyStore, ProviderClient, ProviderKind, ResolvedProvider};
use installer_core::{
    AllowedBinary, CoreConfig, EventBus,
    PlanSafetyReport, Planner, PlatformOsProvider, RecipeStore,
    SafetyEngine, StateLogger, SystemProfile, SystemScanner, OsProvider, Plan,
};

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
    pub current_plan: Arc<RwLock<Option<(Plan, PlanSafetyReport)>>>,
}

/// Reads provider keys per request (local `.env`, then process environment, then startup config)
/// so a key added to `.env` works without a restart.
struct LocalKeyStore {
    startup: ConfigKeyStore,
}

impl KeyStore for LocalKeyStore {
    fn api_key(&self, kind: ProviderKind) -> Option<String> {
        kind.key_env_names()
            .iter()
            .find_map(|name| resolve_env_key(name, None))
            .or_else(|| self.startup.api_key(kind).as_deref().and_then(normalize_api_key))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,installer_gui=debug,installer_core=debug".into()),
        )
        .init();

    info!("Initializing SetupPal Zero-Trust Core...");

    let config = CoreConfig::from_env();
    let events = EventBus::default();
    let os_provider: Arc<dyn OsProvider> = Arc::new(PlatformOsProvider::new());
    let scanner = Arc::new(SystemScanner::new(os_provider.clone(), events.clone()));
    let safety_engine = Arc::new(SafetyEngine::with_system_defaults(os_provider.clone(), config.tavily_api_key.clone(), events.clone()));
    let recipes = RecipeStore::load_from_dir_or_embedded(&config.recipes_dir)?;
    let keys = Arc::new(LocalKeyStore { startup: ConfigKeyStore::from_config(&config) });
    let llm = Arc::new(ProviderClient::with_key_store(config.clone(), keys)?);
    let planner = Arc::new(Planner::new(recipes.clone(), llm.clone(), events.clone()));
    let state_logger = StateLogger::new(config.state_log_path.clone(), events.clone());

    let app_state = AppState {
        config,
        events,
        os_provider,
        scanner,
        safety_engine,
        recipes,
        llm,
        planner,
        state_logger,
        current_plan: Arc::new(RwLock::new(None)),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT]);

    let app = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/doctor", get(doctor_handler))
        .route("/api/recipes", get(recipes_handler))
        .route("/api/chat/stream", post(chat_stream_handler))
        .route("/api/plan", post(plan_handler))
        .route("/api/execute", post(execute_handler))
        .route("/api/history", get(history_handler))
        .route("/api/history/export", get(history_export_handler))
        .route("/api/undo", post(undo_handler))
        .route("/api/events", get(events_handler))
        .route("/api/test-key", post(test_provider_key_handler))
        .layer(cors)
        .with_state(app_state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("🚀 SetupPal Zero-Trust Backend running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    if let Err(e) = axum::serve(listener, app).await {
        error!("Axum server error: {:?}", e);
    }

    Ok(())
}

fn read_dotenv_value(name: &str) -> Option<String> {
    let content = std::fs::read_to_string(".env").ok()?;
    for line in content.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        if let Some(val) = line.strip_prefix(&format!("{name}=")) {
            if let Some(clean) = normalize_api_key(val) {
                return Some(clean);
            }
        }
    }
    None
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

async fn health_handler(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let has_tavily_key =
        resolve_env_key("TAVILY_API_KEY", state.config.tavily_api_key.as_deref()).is_some();

    let mut health = serde_json::json!({
        "status": "ok",
        "app": "SetupPal Zero-Trust Backend",
        "version": "0.1.0",
        "safety_level": "strict",
        "deterministic_gates": true,
        "has_tavily_key": has_tavily_key,
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

async fn recipes_handler(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
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
        .map(|t| (Some(t.version_output.clone()), Some(t.path.to_string_lossy().to_string())))
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

pub fn create_metadata_for_plan(plan: &Plan, safety_report: &PlanSafetyReport) -> Vec<serde_json::Value> {
    plan.steps.iter().map(|s| {
        let (domain, url) = s.step.source_display();
        let report = safety_report.step_reports.iter().find(|r| r.step_id == s.id);
        let (verdict_title, verdict_desc) = report.map(|r| r.user_facing_translation())
            .unwrap_or(("Verified source", "Passed security checks.".to_string()));
        serde_json::json!({
            "step_id": s.id,
            "source_domain": domain,
            "source_url": url,
            "verdict_title": verdict_title,
            "verdict_desc": verdict_desc,
        })
    }).collect()
}

fn normalize_api_key(raw: &str) -> Option<String> {
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

fn resolve_env_key(name: &str, configured: Option<&str>) -> Option<String> {
    read_dotenv_value(name)
        .or_else(|| std::env::var(name).ok().and_then(|value| normalize_api_key(&value)))
        .or_else(|| configured.and_then(normalize_api_key))
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
    let kind = match req.provider.as_deref() {
        Some(value) => ProviderKind::parse(value),
        None => state.llm.selected().ok(),
    };
    let Some(kind) = kind else {
        return Json(TestKeyResponse {
            valid: false,
            message: "Unknown provider. Use `nebius` or `nim`.".to_string(),
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
                "No key to test. Paste one, or add {} to your local environment.",
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
const PROMPT_VERSION: &str = "v21-pixel-broad-scope";

const STATIC_SYSTEM_PROMPT: &str = "\
Role\n\
You are Pixel, SetupPal's assistant: a friendly little ghost who helps people set up software on their Windows PC. Be playful, curious, and warm, with a light spooky joke now and then.\n\
\n\
Scope\n\
SetupPal can set up anything that installs unattended from the command line (silent winget, npm, pip, cargo, or CLI installers like Fabric for Minecraft Java), can be verified with a command such as `node -v`, and passes SetupPal's safety checks. That includes everyday apps, not just coding tools (for example Git, VS Code, 7-Zip, VLC, OBS Studio).\n\
Apps that need a GUI launcher, store login, or DRM (Steam, Epic Games, Riot, Battle.net, Roblox, Minecraft Bedrock, and similar) are out of scope.\n\
\n\
How to work\n\
- Answer the user's latest message, using the whole conversation. Short replies like `php` or `the second one` continue the current topic.\n\
- Each message comes with facts about this PC. Use them and never invent facts. If something is already installed, say so and give its version.\n\
- Greeting or broad request (like \"set up my PC\"): ask what they want set up, and offer 2 to 4 concrete options that fit this PC, recommending one.\n\
- Clear request: give the plan right away. Ask only when the answer changes the plan (for example, which version).\n\
- A plan is a short numbered list: what gets installed, its source (winget, npm, pip, ...), and the command that checks it. Then say nothing is installed until they approve.\n\
\n\
Manual setup guide\n\
If the request is out of scope, start with **Manual setup guide**. In one line, say SetupPal only does unattended terminal setup and why this app needs a manual install. Give short numbered steps using the official source. End with \"SetupPal can install instead:\" and 2 to 4 related options.\n\
\n\
Style\n\
Plain words, short replies, markdown allowed, bold the key point. Don't introduce yourself unless asked. Never claim you installed or checked anything; SetupPal does that after approval. Never send an empty reply.";

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
            let ver = t.version_output.lines().next().unwrap_or("").trim().replace('"', "'");
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
    let launcher_requested = ["minecraft", "bedrock", "steam", "riot", "epic", "launcher", "optifine", "roblox"]
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
        let head = if trimmed_any { "(oldest notes dropped)\n" } else { "" };
        Some(format!("{}{}", head, lines.join("\n")))
    };

    let recent = recent
        .iter()
        .map(|m| ChatMessageItem {
            role: if m.role == "user" { "user".into() } else { "assistant".into() },
            content: clip(&m.content, if m.role == "user" { 700 } else { 900 }),
        })
        .collect();

    PromptContext { memory_note, recent, compacted: older.len() }
}

type ChatSseSender = mpsc::Sender<Result<Event, Infallible>>;

async fn chat_stream_handler(
    State(state): State<AppState>,
    Json(mut req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let raw_msg = req.message.trim().to_string();
    if raw_msg.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Message cannot be empty.".to_string()));
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
            Ok(reply) => {
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
    let data = serde_json::to_string(&payload).map_err(|_| "Could not encode stream event.".to_string())?;
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
        "content": format!("Facts about this PC:\n{host_facts}\n\nUser message:\n{user_prompt}")
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
) -> Result<String, String> {
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
                    let retryable = status == StatusCode::TOO_MANY_REQUESTS
                        || status.is_server_error();
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
    let mut reply = String::new();
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
            if consume_chat_sse_line(&line, &mut data_lines, &mut reply, sender).await? {
                complete = true;
                break;
            }
        }
        if complete {
            break;
        }
    }

    if !complete && !pending.is_empty() {
        consume_chat_sse_line(&pending, &mut data_lines, &mut reply, sender).await?;
    }
    if !complete && !data_lines.is_empty() {
        let final_line = [];
        consume_chat_sse_line(&final_line, &mut data_lines, &mut reply, sender).await?;
    }
    if !reply.chars().any(char::is_alphanumeric) {
        return Err("AI provider returned no usable reply text. Please try again.".to_string());
    }
    Ok(reply)
}

async fn consume_chat_sse_line(
    line: &[u8],
    data_lines: &mut Vec<String>,
    reply: &mut String,
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
            reply.push_str(text);
            send_chat_event(sender, "message.delta", serde_json::json!({ "text": text })).await?;
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

    let plan = state
        .planner
        .create_plan(&req.goal, &profile, req.untrusted_readme.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let safety_report = state
        .safety_engine
        .evaluate_plan(&plan)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let step_metadata = create_metadata_for_plan(&plan, &safety_report);
    state.events.emit(installer_core::AgentEvent::PlanGenerated {
        plan: Box::new(plan.clone()),
    });

    state.events.emit(installer_core::AgentEvent::SafetyCheckCompleted {
        report: Box::new(safety_report.clone()),
    });

    // Cache current plan for subsequent execute calls
    *state.current_plan.write().await = Some((plan.clone(), safety_report.clone()));

    Ok(Json(serde_json::json!({
        "plan": plan,
        "safety": safety_report,
        "profile": profile,
        "step_metadata": step_metadata,
    })))
}

#[derive(Debug, Deserialize)]
struct ExecuteRequest {
    #[allow(dead_code)]
    plan_id: Option<String>,
    dry_run: Option<bool>,
    auto_approve_green: Option<bool>,
    allow_yellow: Option<bool>,
    custom_plan: Option<Plan>,
    trusted_categories: Option<Vec<installer_core::TrustCategory>>,
}

async fn execute_handler(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let (plan, safety_report) = match req.custom_plan {
        Some(p) => {
            let s = state
                .safety_engine
                .evaluate_plan(&p)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            (p, s)
        }
        None => {
            let guard = state.current_plan.read().await;
            match guard.as_ref() {
                Some((p, s)) => (p.clone(), s.clone()),
                None => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "No plan to execute. Ask for a plan first.".to_string(),
                    ))
                }
            }
        }
    };

    info!("Executing plan '{}': {}", plan.plan_id, plan.goal);

    let verifier = installer_core::Verifier::new(state.os_provider.clone(), state.events.clone());
    let adapters = installer_core::AdapterRegistry::with_system_defaults(state.os_provider.clone());
    let executor = installer_core::Executor::new(
        adapters,
        state.os_provider.clone(),
        state.state_logger.clone(),
        verifier,
        state.events.clone(),
    );

    let options = installer_core::ExecutionOptions {
        dry_run: req.dry_run.unwrap_or(false),
        auto_approve_green: req.auto_approve_green.unwrap_or(true),
        allow_yellow: req.allow_yellow.unwrap_or(false),
        working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        trusted_categories: req
            .trusted_categories
            .unwrap_or_default()
            .into_iter()
            .collect(),
    };

    let summary = executor
        .execute_plan(&plan, &safety_report, &options)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "plan": plan,
        "safety": safety_report,
        "summary": summary,
    })))
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
        Ok(agent_event) => {
            match serde_json::to_string(&agent_event) {
                Ok(json) => Some(Ok(Event::default().data(json))),
                Err(e) => {
                    error!("Failed to serialize AgentEvent: {}", e);
                    None
                }
            }
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(test)]
mod provider_selection_tests {
    use super::*;

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
        assert_eq!(llm.resolve().expect("resolves").model, CoreConfig::NIM_MODEL);
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
        ChatMessageItem { role: role.to_string(), content: content.to_string() }
    }

    #[test]
    fn system_prompt_is_pixel_only_and_short() {
        assert!(STATIC_SYSTEM_PROMPT.contains("You are Pixel"));
        assert!(!STATIC_SYSTEM_PROMPT.contains("Byte"));
        assert!(!STATIC_SYSTEM_PROMPT.contains("Mochi"));
        assert!(STATIC_SYSTEM_PROMPT.contains("Manual setup guide"));
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
        assert!(last["content"].as_str().unwrap_or_default().ends_with("User message:\nhi"));
    }
}
