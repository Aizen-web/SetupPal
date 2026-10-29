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
use tokio::sync::RwLock;
use tokio_stream::wrappers::BroadcastStream;
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
        .route("/api/plan", post(plan_handler))
        .route("/api/execute", post(execute_handler))
        .route("/api/history", get(history_handler))
        .route("/api/history/export", get(history_export_handler))
        .route("/api/undo", post(undo_handler))
        .route("/api/events", get(events_handler))
        .route("/api/test-key", post(test_key_handler))
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

fn read_dotenv_nim_key() -> Option<String> {
    let content = std::fs::read_to_string(".env").ok()?;
    for line in content.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        if let Some(val) = line.strip_prefix("NIM_API_KEY=") {
            if let Some(clean) = normalize_api_key(val) {
                return Some(clean);
            }
        }
    }
    None
}

async fn health_handler(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let env_key = read_dotenv_nim_key()
        .or_else(|| state.config.nim_api_key.clone().filter(|s| !s.trim().is_empty()))
        .or_else(|| std::env::var("NIM_API_KEY").ok().filter(|s| !s.trim().is_empty()));

    let has_key = env_key.is_some();
    let masked_key = env_key.map(|k| {
        if k.len() > 10 {
            format!("{}••••••••{}", &k[..6], &k[k.len() - 4..])
        } else {
            "••••••••".to_string()
        }
    });

    Json(serde_json::json!({
        "status": "ok",
        "app": "SetupPal Zero-Trust Backend",
        "version": "0.1.0",
        "safety_level": "strict",
        "deterministic_gates": true,
        "has_env_key": has_key,
        "env_key_masked": masked_key,
        "provider": "nim",
        "model": AI_MODEL
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

/// Single NVIDIA NIM endpoint + model used for chat. Model must be enabled on the key's account.
pub const AI_ENDPOINT: &str = "https://integrate.api.nvidia.com/v1/chat/completions";
pub const AI_MODEL: &str = "nvidia/nemotron-3.5-bolt-e4b";

#[derive(Debug, Clone)]
pub struct ResolvedAiConfig {
    pub api_key: String,
    pub endpoint: String,
    pub model: String,
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

fn resolve_ai_config(req: &ChatRequest, state_config: &installer_core::CoreConfig) -> Option<ResolvedAiConfig> {
    // Prefer a live `.env` read so rotating the key does not require a backend restart,
    // then process env / startup config, then a key saved in the GUI.
    let api_key = read_dotenv_nim_key()
        .or_else(|| std::env::var("NIM_API_KEY").ok().and_then(|s| normalize_api_key(&s)))
        .or_else(|| state_config.nim_api_key.as_deref().and_then(normalize_api_key))
        .or_else(|| req.api_key.as_deref().and_then(normalize_api_key))?;

    Some(ResolvedAiConfig {
        api_key,
        endpoint: AI_ENDPOINT.to_string(),
        model: AI_MODEL.to_string(),
    })
}

#[derive(Debug, Deserialize)]
pub struct TestKeyRequest {
    pub api_key: String,
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

/// Bump when the prompt or host-facts format changes so the session cache never serves old-style replies.
const PROMPT_VERSION: &str = "v16-cli-scope";

const STATIC_SYSTEM_PROMPT: &str = "\
You are SetupPal, a friendly assistant that lives on the user's Windows PC and helps them install and set up developer tools, coding languages, web environments, and software safely.\n\
\n\
Scope and requirements:\n\
- You ONLY install and configure software that supports unattended Command-Line Interface (CLI) setup (e.g. web frameworks, coding languages, compilers, runtimes, SDKs, package managers like winget/npm/pip/cargo, and developer utilities like Git, Node.js, Python, Rust, Go, C++, Docker, VS Code).\n\
- Apps requiring proprietary GUI launchers, interactive store account logins, or DRM wrappers (such as games with launchers like Minecraft Bedrock, Steam, Riot, Epic) CANNOT be installed. If the user asks for these, state clearly and politely that SetupPal only installs things that support unattended CLI setup, explain why (launcher apps cannot be verified or run headlessly via CLI), and recommend relevant CLI or developer tools instead.\n\
\n\
How you work:\n\
- Nothing is installed or changed until the user approves a plan. For now you chat, answer questions, and propose what to do.\n\
- You are given facts about this PC. Use them when they matter, and never invent anything that is not there. If you do not know something, say so.\n\
- Keep replies short, clear, and in plain language. Bold the key point. No emoji.\n\
- Only talk about software the user mentioned. Do not suggest unrelated products.\n\
\n\
What to do:\n\
- Only if the user says hi or asks who you are or what you can do: reply in 1 to 2 sentences, say you can install and set up developer tools, coding languages, and web environments that support unattended CLI setup, and ask what they want to set up.\n\
- For every other message, do NOT greet, do NOT introduce yourself, and do NOT ask what they want to set up. Start directly with the answer.\n\
- Questions about their PC: answer directly from the facts.\n\
- Clear setup request (e.g. 'install Git' or 'set up Node.js'): do not interrogate. State what you will do, mention anything relevant already on the PC, and ask only about real decisions. If there is no real decision, ask nothing and say what you will plan next.\n\
- Broad or vague request (e.g. 'set up C++', 'make my PC ready for coding', 'web dev'): first say what you found on the PC, then offer a short menu of 2 to 4 concrete things you can set up for that topic, each named specifically (for example a compiler, an editor or IDE, a build tool, a package manager) with a few words on what it is for. Mark the best default Recommended. Ask which they want; let them pick several.\n\
- Follow-up answers: when the user picks an option, move forward with it. Never repeat a question they already answered, and never go back to the broad menu.\n\
- Ask 1 question when one decision is open, up to 3 when several are open, and 0 when nothing is. Every option must be a complete, specific choice the user can click as-is (like `Visual Studio Code` or `MSVC via Visual Studio Build Tools`), never a fragment of a sentence.\n\
- If something is already installed, say so and only offer to update or add to it if that is useful.\n\
\n\
Formatting (the app renders markdown). For setup requests you may use these blocks when they help; skip them otherwise:\n\
\n\
### System check\n\
- **Git**: `2.50.1` (Status: Verified) — already installed\n\
(Status is one of: Verified, Attention, Missing, Skipped.)\n\
\n\
### Your choices\n\
1. **Editor**: `Visual Studio Code` (Recommended) — light and popular | `Visual Studio` — full IDE, heavier\n\
\n\
The blocks above are a format example only, never reuse their content. Write real labels and options for the task at hand.\n\
Optionally end with: Next: one short sentence about what happens after they choose.\n\
\n\
Personality: stay in character, but keep it light. A short playful touch in your opening or closing line is plenty, never in the facts or options.";

fn pet_persona(pet: Option<&str>) -> &'static str {
    match pet.unwrap_or("ghost") {
        "cat" => "Your character: Byte, a cool, dry-witted cyber cat. Calm and concise, a little smug but kind.",
        "bunny" => "Your character: Mochi, a sweet, bubbly bunny. Warm, soft, and encouraging.",
        _ => "Your character: Pixel, a playful, curious little ghost. Friendly, a bit shy, gently spooky-cute.",
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

fn distill_history(raw_history: &[ChatMessageItem]) -> (Vec<ChatMessageItem>, usize) {
    let mut compacted_count = 0;
    // Cap history window to the 3 most recent turns to avoid quadratic token growth (O(N^2))
    let slice = if raw_history.len() > 3 {
        compacted_count += raw_history.len() - 3;
        &raw_history[raw_history.len() - 3..]
    } else {
        raw_history
    };

    let items: Vec<ChatMessageItem> = slice.iter().map(|item| {
        let trimmed = item.content.trim();
        if item.role == "assistant" {
            // Cut off trailing question lists or option dumps from previous turns
            let cut = if let Some(idx) = trimmed.find("\n### Your choices") {
                compacted_count += 1;
                &trimmed[..idx]
            } else if let Some(idx) = trimmed.find("\n\n❓ **Questions") {
                compacted_count += 1;
                &trimmed[..idx]
            } else if let Some(idx) = trimmed.find("\n\nReview the") {
                compacted_count += 1;
                &trimmed[..idx]
            } else {
                trimmed
            };

            let distilled = if cut.len() > 220 {
                compacted_count += 1;
                let boundary = cut.floor_char_boundary(220);
                format!("{}...", &cut[..boundary])
            } else {
                cut.to_string()
            };

            ChatMessageItem {
                role: "assistant".to_string(),
                content: distilled,
            }
        } else {
            let content = if trimmed.len() > 250 {
                compacted_count += 1;
                let boundary = trimmed.floor_char_boundary(250);
                format!("{}...", &trimmed[..boundary])
            } else {
                trimmed.to_string()
            };
            ChatMessageItem {
                role: "user".to_string(),
                content,
            }
        }
    }).collect();

    (items, compacted_count)
}

async fn call_ai_chat(
    ai_config: &ResolvedAiConfig,
    user_prompt: &str,
    history: &[ChatMessageItem],
    host_facts: &str,
    pet: Option<&str>,
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

    // 2. Distilled, bounded conversation turns (preventing trailing duplicate user message)
    for (idx, item) in history.iter().enumerate() {
        if idx == history.len().saturating_sub(1) && item.role == "user" && item.content.trim() == user_prompt.trim() {
            continue;
        }
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
        "max_tokens": 800
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
            "HTTP 401 Unauthorized: API key was rejected. Check NIM_API_KEY in .env. Detail: {short}"
        );
    }
    format!("AI Provider returned HTTP {status}: {short}")
}

async fn chat_handler(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    let raw_msg = req.message.trim();

    info!("Received chat message: {:?}", raw_msg);

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
    };

    let Some(ai_config) = resolve_ai_config(&req, &state.config) else {
        return Ok(Json(error_response(
            "**Error: no API key set.** Add your NVIDIA API key in **Settings** (or `NIM_API_KEY` in `.env`).".to_string(),
            true,
        )));
    };

    // Include a lightweight history signature so multi-turn short answers don't collide
    let hist_sig = req.history.as_deref().map(|h| {
        h.iter().map(|i| format!("{}:{}", i.role, i.content.chars().take(20).collect::<String>())).collect::<Vec<_>>().join(";")
    }).unwrap_or_default();
    let cache_key = format!(
        "{}:{}:{}:{}:{}:{}",
        PROMPT_VERSION,
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
                }));
            }
        }
    }

    state.events.emit(installer_core::AgentEvent::PlanningStarted {
        goal: raw_msg.to_string(),
    });

    let raw_hist = req.history.as_deref().unwrap_or(&[]);
    let host_facts = format_host_facts(&profile, &mc_info, raw_msg, raw_hist);
    let (distilled_history, _) = distill_history(raw_hist);

    match call_ai_chat(&ai_config, raw_msg, &distilled_history, &host_facts, req.pet.as_deref()).await {
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
