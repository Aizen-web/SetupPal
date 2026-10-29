//! SetupPal Zero-Trust Backend API & SSE Server
//!
//! Exposes installer_core domain logic to the modern UI frontend via HTTP & Server-Sent Events (SSE).

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
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

use installer_core::{
    AllowedBinary, CoreConfig, EventBus,
    NimClient, PlanSafetyReport, Planner, PlatformOsProvider, RecipeStore,
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
    pub llm: Arc<NimClient>,
    pub planner: Arc<Planner>,
    pub state_logger: StateLogger,
    pub current_plan: Arc<RwLock<Option<(Plan, PlanSafetyReport)>>>,
    pub ai_cache: Arc<RwLock<std::collections::HashMap<String, (String, std::time::Instant)>>>,
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
    let llm = Arc::new(NimClient::new(config.clone())?);
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
        ai_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT]);

    let app = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/doctor", get(doctor_handler))
        .route("/api/recipes", get(recipes_handler))
        .route("/api/chat", post(chat_handler))
        .route("/api/chat/stream", post(chat_stream_handler))
        .route("/api/plan", post(plan_handler))
        .route("/api/execute", post(execute_handler))
        .route("/api/history", get(history_handler))
        .route("/api/history/export", get(history_export_handler))
        .route("/api/undo", post(undo_handler))
        .route("/api/events", get(events_handler))
        .route("/api/test-key", post(test_provider_key_handler))
        .route("/api/models", get(models_handler))
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

fn read_dotenv_nim_key() -> Option<String> {
    read_dotenv_value("NIM_API_KEY").or_else(|| read_dotenv_value("NVIDIA_API_KEY"))
}

async fn health_handler(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let has_key = resolve_nim_api_key(state.config.nim_api_key.as_deref()).is_some();
    let has_nebius_key =
        resolve_env_key("NEBIUS_API_KEY", state.config.nebius_api_key.as_deref()).is_some();
    let has_tavily_key =
        resolve_env_key("TAVILY_API_KEY", state.config.tavily_api_key.as_deref()).is_some();

    Json(serde_json::json!({
        "status": "ok",
        "app": "SetupPal Zero-Trust Backend",
        "version": "0.1.0",
        "safety_level": "strict",
        "deterministic_gates": true,
        "has_env_key": has_key,
        "has_nebius_key": has_nebius_key,
        "has_tavily_key": has_tavily_key,
        "provider": if has_nebius_key { "nebius" } else { "nim" },
        "model": state.config.nebius_model.as_deref().unwrap_or(&state.config.nim_model)
    }))
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

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub history: Option<Vec<ChatMessageItem>>,
    pub api_key: Option<String>,
    /// Active mascot: "ghost" | "cat" | "bunny". Drives the assistant's personality.
    pub pet: Option<String>,
    /// How many recent messages are sent word for word (older ones are compacted into notes).
    pub memory_turns: Option<usize>,
    /// Max tokens for the reply.
    pub max_reply_tokens: Option<u32>,
    /// Whether to attach PC facts to the message.
    pub include_pc_facts: Option<bool>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatOption {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenOptimizationMetrics {
    pub prompt_cache_hit: bool,
    pub history_messages_compacted: usize,
    pub estimated_tokens_saved: usize,
    pub optimization_strategy: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub reply: String,
    pub options: Vec<ChatOption>,
    pub plan: Option<Plan>,
    pub safety: Option<PlanSafetyReport>,
    pub step_metadata: Option<Vec<serde_json::Value>>,
    pub ready_to_execute: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_metrics: Option<TokenOptimizationMetrics>,
    /// What was sent to the model this turn (powers the context viewer).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
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

#[derive(Debug, Clone)]
pub struct ResolvedAiConfig {
    pub api_key: String,
    pub endpoint: String,
    pub model: String,
    pub provider: String,
}

pub const AI_ENDPOINT: &str = "https://integrate.api.nvidia.com/v1/chat/completions";
pub const AI_MODEL: &str = "nvidia/nemotron-3.5-bolt-e4b";

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

fn resolve_nim_api_key(configured: Option<&str>) -> Option<String> {
    resolve_env_key("NIM_API_KEY", configured)
        .or_else(|| resolve_env_key("NVIDIA_API_KEY", None))
}

fn prefers_nebius(requested_provider: Option<&str>, has_nebius_key: bool) -> bool {
    match requested_provider {
        Some("nebius") => true,
        Some("nim") => false,
        _ => has_nebius_key,
    }
}

fn resolve_ai_config(
    req: &ChatRequest,
    state_config: &installer_core::CoreConfig,
) -> Option<ResolvedAiConfig> {
    let has_nebius_key =
        resolve_env_key("NEBIUS_API_KEY", state_config.nebius_api_key.as_deref()).is_some();
    let use_nebius = prefers_nebius(req.provider.as_deref(), has_nebius_key);

    let (api_key, base_url, model, provider) = if use_nebius {
        (
            resolve_env_key("NEBIUS_API_KEY", state_config.nebius_api_key.as_deref())?,
            req.endpoint
                .clone()
                .unwrap_or_else(|| state_config.nebius_base_url.clone()),
            req.model
                .clone()
                .or_else(|| state_config.nebius_model.clone())
                .unwrap_or_default(),
            "nebius",
        )
    } else {
        (
            resolve_nim_api_key(state_config.nim_api_key.as_deref())
                .or_else(|| req.api_key.as_deref().and_then(normalize_api_key))?,
            req.endpoint
                .clone()
                .unwrap_or_else(|| state_config.nim_base_url.clone()),
            req.model.clone().unwrap_or_else(|| state_config.nim_model.clone()),
            "nim",
        )
    };

    Some(ResolvedAiConfig {
        api_key,
        endpoint: format!("{}/chat/completions", base_url.trim_end_matches('/')),
        model,
        provider: provider.to_string(),
    })
}

#[derive(Debug, Deserialize)]
pub struct TestKeyRequest {
    pub api_key: String,
    pub provider: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TestKeyResponse {
    pub valid: bool,
    pub message: String,
    pub model_tested: String,
    pub provider: String,
}

async fn test_key_handler(
    Json(req): Json<TestKeyRequest>,
) -> Json<TestKeyResponse> {
    let provider = "NVIDIA NIM".to_string();
    let model = AI_MODEL.to_string();

    let key = if req.api_key.trim().is_empty() {
        read_dotenv_nim_key().unwrap_or_default()
    } else {
        req.api_key.trim().to_string()
    };

    if key.is_empty() {
        return Json(TestKeyResponse {
            valid: false,
            message: "No API key found in request or .env file.".to_string(),
            model_tested: model,
            provider,
        });
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build() {
        Ok(c) => c,
        Err(e) => {
            return Json(TestKeyResponse {
                valid: false,
                message: format!("HTTP client initialization failed: {e}"),
                model_tested: model,
                provider,
            });
        }
    };

    let res = client
        .post(AI_ENDPOINT)
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": "ping" }],
            "max_tokens": 5,
            "temperature": 0.0
        }))
        .send()
        .await;

    match res {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.is_success() {
                Json(TestKeyResponse {
                    valid: true,
                    message: format!("Key works. Model '{model}' responded."),
                    model_tested: model,
                    provider,
                })
            } else {
                Json(TestKeyResponse {
                    valid: false,
                    message: format_nvidia_http_error(status, &body, &model),
                    model_tested: model,
                    provider,
                })
            }
        }
        Err(err) => Json(TestKeyResponse {
            valid: false,
            message: format!("Network request failed: {err}"),
            model_tested: model,
            provider,
        }),
    }
}

async fn test_provider_key_handler(
    State(state): State<AppState>,
    Json(req): Json<TestKeyRequest>,
) -> Json<TestKeyResponse> {
    let use_nebius = req.provider.as_deref() == Some("nebius");
    let provider = if use_nebius { "Nebius" } else { "NVIDIA NIM" }.to_string();
    let base_url = if use_nebius {
        &state.config.nebius_base_url
    } else {
        &state.config.nim_base_url
    };
    let model = if use_nebius {
        state.config.nebius_model.clone().unwrap_or_default()
    } else {
        state.config.nim_model.clone()
    };
    let key = if req.api_key.trim().is_empty() {
        let (name, configured) = if use_nebius {
            ("NEBIUS_API_KEY", state.config.nebius_api_key.as_deref())
        } else {
            ("NIM_API_KEY", state.config.nim_api_key.as_deref())
        };
        if use_nebius {
            resolve_env_key(name, configured).unwrap_or_default()
        } else {
            resolve_nim_api_key(configured).unwrap_or_default()
        }
    } else {
        normalize_api_key(&req.api_key).unwrap_or_default()
    };
    if key.is_empty() {
        return Json(TestKeyResponse {
            valid: false,
            message: "No API key found in the request or environment.".to_string(),
            model_tested: if model.is_empty() { "Live model list".to_string() } else { model },
            provider,
        });
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            return Json(TestKeyResponse {
                valid: false,
                message: "HTTP client initialization failed.".to_string(),
                model_tested: if model.is_empty() { "Live model list".to_string() } else { model },
                provider,
            });
        }
    };

    match client
        .get(format!("{}/models", base_url.trim_end_matches('/')))
        .bearer_auth(&key)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => Json(TestKeyResponse {
            valid: true,
            message: "Key works. Model list is available.".to_string(),
            model_tested: if model.is_empty() { "Live model list".to_string() } else { model },
            provider,
        }),
        Ok(response) => Json(TestKeyResponse {
            valid: false,
            message: format!("Provider returned HTTP {}. Check the key and endpoint.", response.status()),
            model_tested: if model.is_empty() { "Live model list".to_string() } else { model },
            provider,
        }),
        Err(error) => Json(TestKeyResponse {
            valid: false,
            message: format!("Network request failed: {}", error.without_url()),
            model_tested: if model.is_empty() { "Live model list".to_string() } else { model },
            provider,
        }),
    }
}

#[derive(Debug, Deserialize)]
struct ModelsQuery {
    provider: Option<String>,
}

async fn models_handler(
    State(state): State<AppState>,
    Query(query): Query<ModelsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let use_nebius = query.provider.as_deref() == Some("nebius");
    let (provider, base_url, configured_key) = if use_nebius {
        ("nebius", &state.config.nebius_base_url, state.config.nebius_api_key.as_deref())
    } else {
        ("nim", &state.config.nim_base_url, state.config.nim_api_key.as_deref())
    };
    let key_name = if use_nebius { "NEBIUS_API_KEY" } else { "NIM_API_KEY" };
    let key = if use_nebius {
        resolve_env_key(key_name, configured_key)
    } else {
        resolve_nim_api_key(configured_key)
    }
    .ok_or((StatusCode::UNAUTHORIZED, "No provider key is configured.".to_string()))?;
    let models = fetch_model_ids(base_url, &key).await
        .map_err(|message| (StatusCode::BAD_GATEWAY, message))?;
    Ok(Json(serde_json::json!({ "provider": provider, "models": models })))
}

async fn fetch_model_ids(base_url: &str, api_key: &str) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| "Could not initialize the model-list client.".to_string())?;
    let response = client
        .get(format!("{}/models", base_url.trim_end_matches('/')))
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|error| format!("Could not retrieve models: {}", error.without_url()))?;
    if !response.status().is_success() {
        return Err(format!("Model service returned HTTP {}.", response.status()));
    }
    let payload: serde_json::Value = response.json().await
        .map_err(|_| "Model service returned an invalid response.".to_string())?;
    let models = payload.get("data")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(serde_json::Value::as_str))
        .map(str::to_string)
        .collect();
    Ok(models)
}

/// Bump when the prompt or host-facts format changes so the session cache never serves old-style replies.
const PROMPT_VERSION: &str = "v20-mascot-personalities";

const STATIC_SYSTEM_PROMPT: &str = "\
You are SetupPal's chat assistant on the user's Windows PC. The active mascot identity and speaking style are provided separately. Help the user plan developer software: programming languages, runtimes, compilers, SDKs, package managers, web stacks, and dev tools.\n\
\n\
Scope: you only install things that can be set up unattended from the command line (winget, npm, pip, cargo, etc.) and checked with a CLI command like `node -v`. Apps that need a GUI launcher, store login, or DRM (Steam, Epic, Minecraft Bedrock, and similar) are out of scope. If asked, say so briefly and suggest a CLI or developer alternative.\n\
\n\
How to work:\n\
- Read the whole conversation and answer what the user actually said. Their latest message is the priority. Short replies like `php` or `the second one` continue the topic you were already on.\n\
- You receive facts about this PC. Use them and never invent facts. If a tool is already installed, say so and give its version.\n\
- Your job is to get to a concrete install plan quickly. Work out what is needed, what is already present, and what is missing. Ask a question only when a real decision changes the plan (for example which version, or which of several tools). If the request is clear, skip questions and state the plan.\n\
- If the request is vague, suggest 2 to 4 specific things you can set up and recommend one.\n\
- When you have enough information, give the plan as a short numbered list of install steps, each with the tool name and how it will be verified. Then say the user can approve it to begin. Nothing is installed until they approve.\n\
- If the user greets you, reply with a brief greeting in your mascot's voice and ask what they would like to set up. Never return an empty answer. Do not introduce yourself unless asked, and never repeat a question that was already answered.\n\
\n\
Style: plain language, short, markdown allowed, bold the key point. Keep the mascot voice in the wording, but keep technical facts and safety decisions accurate. The grumpy mascot may be dry or playfully blunt, but must never demean, threaten, or harass the user.";

fn pet_persona(pet: Option<&str>) -> &'static str {
    match pet {
        Some("cat") => {
            "Mascot: Byte the cat. Personality: nonchalant, dry, mildly grumpy, and clever. Use concise deadpan humor and occasional playful bluntness; never insult or demean the user."
        }
        Some("bunny") => {
            "Mascot: Mochi the bunny. Personality: timid, gentle, and earnest. Speak softly, sometimes a little hesitant, while staying clear, useful, and confident about safety."
        }
        _ => {
            "Mascot: Pixel the ghost. Personality: playful, curious, and warm. Add light, slightly spooky humor without letting it obscure the answer."
        }
    }
}

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
        "- OS: {} {} ({})\n- GPU: {}\n- Free disk: {} GB\n- CLI support: winget, user-scope PATH available\n- Supported scope: Unattended CLI setups only (web, coding languages, compilers, dev tools)\n- Installed dev tools: {}",
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
        out.push_str("\n- Note on requested software: The requested application uses a proprietary GUI launcher, store DRM, or interactive account authentication. SetupPal CANNOT install launcher-based apps or store games; remind the user that SetupPal is strictly designed for unattended CLI software, coding languages, web runtimes, and developer toolchains.");
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
            format!("- Pixel: {}", gist(&m.content, 140))
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

async fn call_ai_chat(
    ai_config: &ResolvedAiConfig,
    user_prompt: &str,
    ctx: &PromptContext,
    host_facts: &str,
    pet: Option<&str>,
    max_tokens: u32,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| e.to_string())?;

    let mut messages: Vec<serde_json::Value> = Vec::new();

    // 1. Invariant Static System Prompt (matches OpenAI / Anthropic / vLLM Prompt Cache Prefix)
    messages.push(serde_json::json!({
        "role": "system",
        "content": format!("{}\n\n{}", STATIC_SYSTEM_PROMPT, pet_persona(pet))
    }));

    // 2. Compact notes for older turns, then the most recent turns as written
    if let Some(note) = &ctx.memory_note {
        messages.push(serde_json::json!({
            "role": "system",
            "content": format!("Earlier in this chat (compacted notes, oldest first). Treat what the user said here as still true:\n{}", note)
        }));
    }
    for item in ctx.recent.iter() {
        messages.push(serde_json::json!({
            "role": item.role,
            "content": item.content
        }));
    }

    // 3. Current user message, with the PC facts attached
    let formatted_user_turn = format!(
        "Facts about this PC:\n{}\n\nUser message:\n{}",
        host_facts,
        user_prompt
    );

    messages.push(serde_json::json!({
        "role": "user",
        "content": formatted_user_turn
    }));

    // Explicit max_tokens bounds to prevent runaway output generation
    let request_body = serde_json::json!({
        "model": ai_config.model,
        "messages": messages,
        "temperature": 0.5,
        "max_tokens": max_tokens
    });

    // NVIDIA's hosted endpoint occasionally returns a transient empty 404/429/5xx; retry a couple of times.
    let mut attempt = 0;
    let res = loop {
        attempt += 1;
        let res = client
            .post(&ai_config.endpoint)
            .header("Authorization", format!("Bearer {}", ai_config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Network request to AI endpoint failed: {e}"))?;

        if res.status().is_success() {
            break res;
        }

        let status = res.status();
        let error_body = res.text().await.unwrap_or_default();
        let transient = status.as_u16() == 429
            || status.is_server_error()
            || (status == StatusCode::NOT_FOUND && error_body.trim().is_empty());
        if transient && attempt < 3 {
            tokio::time::sleep(std::time::Duration::from_millis(600 * attempt as u64)).await;
            continue;
        }
        return Err(format_nvidia_http_error(status, &error_body, &ai_config.model));
    };

    let completion: serde_json::Value = res.json().await
        .map_err(|e| format!("Failed to parse AI response JSON: {e}"))?;

    let content = completion
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .ok_or_else(|| "AI response missing content".to_string())?;

    extract_message_content(content)
}

fn extract_message_content(content: &serde_json::Value) -> Result<String, String> {
    if let Some(text) = content.as_str() {
        return Ok(text.to_string());
    }
    // Some NVIDIA models return content as an array of text parts.
    if let Some(parts) = content.as_array() {
        let joined = parts
            .iter()
            .filter_map(|p| {
                p.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| p.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
            })
            .collect::<Vec<_>>()
            .join("");
        if !joined.is_empty() {
            return Ok(joined);
        }
    }
    Err(format!("AI response content had unexpected shape: {content}"))
}

fn format_nvidia_http_error(status: StatusCode, body: &str, model: &str) -> String {
    let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let short = if flat.len() > 320 {
        format!("{}...", &flat[..flat.floor_char_boundary(320)])
    } else {
        flat
    };

    if status == StatusCode::NOT_FOUND && short.contains("Not found for account") {
        return format!(
            "HTTP 404: model `{model}` is not enabled for this API key. \
On build.nvidia.com open that model and click Get API Key / enable it, \
or set NIM_MODEL in .env to a model your key can call. Detail: {short}"
        );
    }
    if status == StatusCode::FORBIDDEN {
        return format!(
            "HTTP 403 Forbidden: NVIDIA rejected this API key for chat inference. \
Confirm it is a build.nvidia.com inference key (nvapi-...), not an expired NGC download key. Detail: {short}"
        );
    }
    if status == StatusCode::GONE || status.as_u16() == 410 {
        return format!(
            "HTTP 410 Gone: model `{model}` has been retired by NVIDIA. Update NIM_MODEL / AI_MODEL. Detail: {short}"
        );
    }
    if status == StatusCode::UNAUTHORIZED {
        return format!(
            "HTTP 401 Unauthorized: API key was rejected. Check NIM_API_KEY or NVIDIA_API_KEY in .env. Detail: {short}"
        );
    }
    format!("AI Provider returned HTTP {status}: {short}")
}

async fn chat_handler(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    let raw_msg = req.message.trim();

    let profile = state
        .scanner
        .scan()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mc_info = inspect_minecraft_system(&profile);

    let error_response = |reply: String, with_settings: bool| ChatResponse {
        reply,
        options: if with_settings {
            vec![ChatOption {
                label: "Open Settings".to_string(),
                value: "open_settings".to_string(),
            }]
        } else {
            vec![]
        },
        plan: None,
        safety: None,
        step_metadata: None,
        ready_to_execute: false,
        token_metrics: None,
        context: None,
    };

    let Some(mut ai_config) = resolve_ai_config(&req, &state.config) else {
        return Ok(Json(error_response(
            "**No AI provider is configured.** Add either `NEBIUS_API_KEY` or an NVIDIA key (`NIM_API_KEY` / `NVIDIA_API_KEY`) to your local `.env` file.".to_string(),
            true,
        )));
    };
    if ai_config.model.is_empty() {
        let models = fetch_model_ids(
            ai_config.endpoint.trim_end_matches("/chat/completions"),
            &ai_config.api_key,
        )
        .await
        .map_err(|message| (StatusCode::BAD_GATEWAY, message))?;
        ai_config.model = models
            .into_iter()
            .find(|model| model.to_ascii_lowercase().contains("nemotron"))
            .ok_or((
                StatusCode::BAD_GATEWAY,
                "No Nemotron model is available for this Nebius account.".to_string(),
            ))?;
    }

    let raw_hist = req.history.as_deref().unwrap_or(&[]);
    let keep_recent = req.memory_turns.unwrap_or(6);
    let max_tokens = req.max_reply_tokens.unwrap_or(800).clamp(200, 2000);
    let include_facts = req.include_pc_facts.unwrap_or(true);
    let ctx = build_context(raw_hist, raw_msg, keep_recent);
    let host_facts = if include_facts {
        format_host_facts(&profile, &mc_info, raw_msg, raw_hist)
    } else {
        "(PC facts are turned off by the user. Do not claim to know what is installed.)".to_string()
    };
    let system_tokens = est_tokens(STATIC_SYSTEM_PROMPT) + est_tokens(pet_persona(req.pet.as_deref()));
    let memory_tokens = ctx.memory_note.as_deref().map(est_tokens).unwrap_or(0);
    let recent_tokens: usize = ctx.recent.iter().map(|m| est_tokens(&m.content)).sum();
    let facts_tokens = est_tokens(&host_facts);
    let user_tokens = est_tokens(raw_msg);
    let context_json = serde_json::json!({
        "model": ai_config.model,
        "system_tokens": system_tokens,
        "memory_tokens": memory_tokens,
        "recent_tokens": recent_tokens,
        "facts_tokens": facts_tokens,
        "user_tokens": user_tokens,
        "total_tokens": system_tokens + memory_tokens + recent_tokens + facts_tokens + user_tokens,
        "reply_budget": max_tokens,
        "total_messages": raw_hist.len(),
        "recent_messages": ctx.recent.len(),
        "compacted_messages": ctx.compacted,
        "memory_note": ctx.memory_note,
        "recent": ctx.recent,
        "facts": host_facts,
    });

    // Include a lightweight history signature so multi-turn short answers don't collide
    let hist_sig = req.history.as_deref().map(|h| {
        h.iter().map(|i| format!("{}:{}", i.role, i.content.chars().take(20).collect::<String>())).collect::<Vec<_>>().join(";")
    }).unwrap_or_default();
    let cache_key = format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}",
        PROMPT_VERSION,
        keep_recent,
        max_tokens,
        include_facts,
        req.pet.as_deref().unwrap_or("ghost"),
        ai_config.model,
        ai_config.endpoint,
        hist_sig,
        raw_msg.to_ascii_lowercase()
    );

    // Session cache of real AI replies only
    {
        let cache = state.ai_cache.read().await;
        if let Some((cached_reply, timestamp)) = cache.get(&cache_key) {
            if timestamp.elapsed() < std::time::Duration::from_secs(600) {
                info!("Serving AI response from session cache");
                return Ok(Json(ChatResponse {
                    reply: cached_reply.clone(),
                    options: vec![],
                    plan: None,
                    safety: None,
                    step_metadata: None,
                    ready_to_execute: false,
                    token_metrics: None,
                    context: Some(context_json.clone()),
                }));
            }
        }
    }

    state.events.emit(installer_core::AgentEvent::PlanningStarted {
        goal: raw_msg.to_string(),
    });


    match call_ai_chat(&ai_config, raw_msg, &ctx, &host_facts, req.pet.as_deref(), max_tokens).await {
        Ok(ai_reply) => {
            {
                let mut cache = state.ai_cache.write().await;
                cache.retain(|_, (_, time)| time.elapsed() < std::time::Duration::from_secs(600));
                if cache.len() >= 100 {
                    if let Some(oldest) = cache.iter().min_by_key(|(_, (_, t))| *t).map(|(k, _)| k.clone()) {
                        cache.remove(&oldest);
                    }
                }
                cache.insert(cache_key, (ai_reply.clone(), std::time::Instant::now()));
            }

            Ok(Json(ChatResponse {
                reply: ai_reply,
                options: vec![],
                plan: None,
                safety: None,
                step_metadata: None,
                ready_to_execute: false,
                token_metrics: None,
                context: Some(context_json.clone()),
            }))
        }
        Err(err) => {
            let err_flat = err.split_whitespace().collect::<Vec<_>>().join(" ");
            let err_short = if err_flat.len() > 400 {
                format!("{}...", &err_flat[..err_flat.floor_char_boundary(400)])
            } else {
                err_flat
            };
            Ok(Json(error_response(
                format!("**Error from `{}`:**\n\n> `{}`", ai_config.model, err_short),
                true,
            )))
        }
    }
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

    let profile = state
        .scanner
        .scan()
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let mc_info = inspect_minecraft_system(&profile);
    req.endpoint = None;
    let mut ai_config = resolve_ai_config(&req, &state.config)
        .ok_or((StatusCode::UNAUTHORIZED, "Add a provider key in the local environment to start chat.".to_string()))?;
    if ai_config.model.is_empty() {
        let models = fetch_model_ids(
            ai_config.endpoint.trim_end_matches("/chat/completions"),
            &ai_config.api_key,
        )
        .await
        .map_err(|message| (StatusCode::BAD_GATEWAY, message))?;
        ai_config.model = models
            .into_iter()
            .find(|model| model.to_ascii_lowercase().contains("nemotron"))
            .ok_or((
                StatusCode::BAD_GATEWAY,
                "No Nemotron model is available for this Nebius account.".to_string(),
            ))?;
    }

    let history = req.history.take().unwrap_or_default();
    let context = build_context(&history, &raw_msg, req.memory_turns.unwrap_or(6));
    let host_facts = if req.include_pc_facts.unwrap_or(true) {
        format_host_facts(&profile, &mc_info, &raw_msg, &history)
    } else {
        "(PC facts are turned off by the user. Do not claim to know what is installed.)".to_string()
    };
    let max_tokens = req.max_reply_tokens.unwrap_or(800).clamp(200, 2000);
    let system_tokens = est_tokens(STATIC_SYSTEM_PROMPT)
        + est_tokens(pet_persona(req.pet.as_deref()));
    let memory_tokens = context.memory_note.as_deref().map(est_tokens).unwrap_or(0);
    let recent_tokens: usize = context
        .recent
        .iter()
        .map(|message| est_tokens(&message.content))
        .sum();
    let facts_tokens = est_tokens(&host_facts);
    let user_tokens = est_tokens(&raw_msg);
    let context_payload = serde_json::json!({
        "model": ai_config.model,
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
            serde_json::json!({ "model": ai_config.model, "provider": ai_config.provider }),
        )
        .await
        .is_err()
        {
            return;
        }

        match call_ai_chat_stream(
            &ai_config,
            &raw_msg,
            &context,
            &host_facts,
            req.pet.as_deref(),
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

async fn call_ai_chat_stream(
    ai_config: &ResolvedAiConfig,
    user_prompt: &str,
    context: &PromptContext,
    host_facts: &str,
    pet: Option<&str>,
    max_tokens: u32,
    sender: &ChatSseSender,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|_| "Could not initialize the AI connection.".to_string())?;

    let mut messages: Vec<serde_json::Value> = vec![serde_json::json!({
        "role": "system",
        "content": format!("{}\n\n{}", STATIC_SYSTEM_PROMPT, pet_persona(pet))
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

    let request_body = serde_json::json!({
        "model": ai_config.model,
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
                .post(&ai_config.endpoint)
                .bearer_auth(&ai_config.api_key)
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
                        return Err(format!("AI provider returned HTTP {status}."));
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

    state.events.emit(installer_core::AgentEvent::PlanningStarted {
        goal: req.goal.clone(),
    });

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
    use super::prefers_nebius;

    #[test]
    fn auto_prefers_nebius_when_a_nebius_key_is_configured() {
        assert!(prefers_nebius(Some("auto"), true));
    }

    #[test]
    fn auto_falls_back_to_nvidia_without_a_nebius_key() {
        assert!(!prefers_nebius(Some("auto"), false));
    }

    #[test]
    fn explicit_provider_choice_is_preserved() {
        assert!(!prefers_nebius(Some("nim"), true));
        assert!(prefers_nebius(Some("nebius"), false));
    }
}
