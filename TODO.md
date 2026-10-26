# SetupPal: Zero-Trust AI Environment & Package Orchestrator
## Implementation Roadmap & Developer Handoff Guide

> **Current Status:** Milestones 1, 2, and 3 are **COMPLETED** and tested. 19 unit tests passing, zero warnings.
> **Repository:** [https://github.com/Aizen-web/SetupPal](https://github.com/Aizen-web/SetupPal)
> **Branch:** `main`

---

## 🏗 System Architecture Overview

SetupPal is an AI-powered installer and environment setup agent with deterministic zero-trust safety gates:
- **CLI (`installer_cli`)** and **Tauri v2 GUI (`installer_gui`)** share 100% of domain logic in **`installer_core`**.
- **Closed Action Schema:** The LLM proposes plans using strictly typed `Step` variants—**never raw shell strings**.
- **Deterministic Safety Engine:** Every step is evaluated against hard policy rules, OSV.dev CVEs, Levenshtein typosquat detection, metadata heuristics, and SHA-256 verification before anything touches the system.
- **Event-Driven:** All state, scan, safety, execution, verification, and healing progress is broadcast via `EventBus` (`tokio::sync::broadcast::Sender<AgentEvent>`).

---

## 📋 Milestone Progress Tracker

- [x] **Gate 0:** Workspace layout, core types (`Plan`, `Step`, `RiskRating`, `SafetyReport`, `AgentEvent`, `SourceAdapter`).
- [x] **Milestone 1:** Workspace scaffolding, config/env loader, `scan` module, NVIDIA NIM client (with `schemars` JSON Schema enforcement + offline mock fallback), and `orchestrator plan "<goal>"`.
- [x] **Milestone 2:** `safety` engine v1 (hard policy filter, Levenshtein typosquat radar, OSV.dev client, SHA-256 verifier, risk scoring, traffic lights, adversarial prompt injection defense).
- [x] **Milestone 3:** `executor` + `verifier` + `state` transaction logger for Windows C++ toolchain setup (`winget` install + Windows Registry `HKCU\Environment` `PATH` refresh + sandbox compiling and running a temporary C++20 `hello.cpp`).
- [ ] **Milestone 4:** Real ecosystem adapters (`PyPI`, `npm`, `Modrinth`, `GitHubReleases`).
- [ ] **Milestone 5:** `recipes` YAML engine + 2 flagship recipes (Python AI/Data Science venv & Minecraft Fabric 1.21 performance mods).
- [ ] **Milestone 6:** `healer` self-healing loop (capture failure diagnostic, NIM remediation plan, route through `safety`, verify recovery).
- [ ] **Milestone 7:** Tauri v2 + Svelte desktop GUI wired to `installer_core` and live `AgentEvent` stream.
- [ ] **Milestone 8:** Full `undo` rollback engine, polish, and scripted `demo-attack` showcase.

---

## 🚀 Remaining Milestones & Implementation Details

### Milestone 4: Multi-Ecosystem Source Adapters
**Goal:** Implement production HTTP source adapters for PyPI, npm, Modrinth, and GitHub Releases to resolve real metadata, download URLs, and cryptographic hashes.

#### Tasks:
1. **PyPI Adapter (`crates/core/src/adapters/pypi.rs`):**
   - Query JSON API: `https://pypi.org/pypi/{package_id}/json`
   - Extract release age from publish timestamp, download count / popularity signals.
   - Extract wheel / sdist URLs and upstream `digests.sha256`.
   - Support `install`: simulate or invoke python `-m pip install {package_id}=={version}` inside active environment.
2. **npm Adapter (`crates/core/src/adapters/npm.rs`):**
   - Query registry API: `https://registry.npmjs.org/{package_id}`
   - Extract versions, publish times, tarball URL, and `dist.shasum` / `dist.integrity`.
   - Support `install`: simulate or invoke `npm install {package_id}@{version}`.
3. **Modrinth Adapter (`crates/core/src/adapters/modrinth.rs`):**
   - Query API: `https://api.modrinth.com/v2/project/{id}/version`
   - Filter by loader (`fabric`) and game version (`1.21`).
   - Extract file download URL, SHA-512, and SHA-256 hashes.
   - Support `fetch_artifact`: download directly to destination and verify hash.
4. **GitHub Releases Adapter (`crates/core/src/adapters/github.rs`):**
   - Query GitHub API: `https://api.github.com/repos/{owner}/{repo}/releases`
   - Filter assets by architecture/OS keyword (`win64`, `x86_64.msi`, `zip`, etc.).
   - Extract release tags, asset URLs, and checksum files if published.
5. **Adapter Registry Wiring:**
   - Register all live adapters in `AdapterRegistry::with_system_defaults(...)`.
   - Ensure unit tests use mock/fixture data so `cargo test` does not require network or perform live system writes.

---

### Milestone 5: YAML Recipe Engine & Flagship Manifests
**Goal:** Load, validate, and execute curated YAML recipes for complex multi-step environments.

#### Tasks:
1. **Dynamic Recipe Loader (`crates/core/src/recipes/`):**
   - Ensure `RecipeStore` parses `.yaml` manifests from both embedded strings (`include_str!`) and custom directories (`ORCHESTRATOR_RECIPES_DIR`).
   - Add trigger phrase matching (fuzzy and exact) against user goal.
2. **Flagship Recipe 1: Python AI/Data Science (`recipes/python_ai_venv.yaml`):**
   - Step 1: Create isolated virtualenv (`Step::CreateIsolatedEnv { kind: PythonVenv, path: "./.venv" }`).
   - Step 2: Install packages: `numpy`, `pandas`, `colorama`.
   - Step 3: Run verify command checking package imports and versions (`python -c "import numpy, pandas, colorama; print('OK')"`).
3. **Flagship Recipe 2: Minecraft Java + Fabric + Performance Mods (`recipes/minecraft_fabric_1_21.yaml`):**
   - Step 1: Install Adoptium Temurin 21 JDK via `winget` (`EclipseAdoptium.Temurin.21.JDK`).
   - Step 2: Download Fabric installer JAR via `OfficialVendorUrl` / `GitHubReleases` with SHA-256 verification.
   - Step 3: Fetch Modrinth mods (`sodium`, `lithium`, `ferrite-core`) with SHA-512 / SHA-256 checks.
   - Step 4: Verify Java runtime version (`java -version`).

---

### Milestone 6: Self-Healing Loop (`healer`)
**Goal:** On execution or verification failure, diagnose the error with NVIDIA NIM, propose a remediation plan, pass it through `SafetyEngine`, and recover automatically.

#### Tasks:
1. **Failure Context Capture (`crates/core/src/healer/`):**
   - Capture `failed_step_id`, exit code, structured stdout, and stderr.
   - Collect updated `SystemProfile` and list of successful previous steps.
2. **NIM Remediation Planner:**
   - Prompt NIM with structured diagnostic context wrapped in delimiters.
   - NIM outputs a remediation `Plan` (`source: PlanSource::Healer { failed_step_id, attempt }`).
3. **Mandatory Zero-Trust Safety Gate:**
   - Remediation plan **MUST** pass through `SafetyEngine::evaluate_plan`.
   - If `RiskRating::Red`, execution is blocked and reported to the user.
4. **Recovery Execution & Loop Limit:**
   - Cap recovery attempts to max 2 attempts to prevent infinite retry loops.
   - Emit `AgentEvent::SelfHealTriggered` and `AgentEvent::SelfHealPlanProposed`.

---

### Milestone 7: Tauri v2 Desktop GUI (`crates/gui`)
**Goal:** Sleek, dark-mode desktop security dashboard powered by Tauri v2, Svelte, and TypeScript.

#### Tasks:
1. **Tauri IPC Commands (`crates/gui/src-tauri/src/main.rs`):**
   - `scan_system()` -> returns `SystemProfile`.
   - `generate_plan(goal: String)` -> returns `Plan` and `PlanSafetyReport`.
   - `execute_plan(options)` -> invokes `Executor`.
   - `get_transaction_log()` -> returns transaction history from `StateLogger`.
   - `undo_transaction(tx_id: Option<String>)` -> executes rollback.
2. **Real-time Event Bridge:**
   - Subscribe Tauri app to `installer_core::EventBus`.
   - Forward all `AgentEvent` variants to the frontend via `app_handle.emit("agent-event", event)`.
3. **Svelte UI Panels (`crates/gui/src/`):**
   - **System Scan Panel:** CPU, GPU, OS version, disk free, detected toolchain chips.
   - **Goal Input & Recipe Quick-Picks:** One-click shortcuts for "C++20 Dev", "Python AI Venv", "Minecraft Fabric".
   - **Plan & Safety Table:** Traffic-light badges (Green/Yellow/Red), risk scores, and expandable drawers showing OSV CVE details, Levenshtein edit distance, and hash verification.
   - **Interactive Step Toggles:** Approve / Deny switches per step; "1-Click Swap to Safe Alternative" button for typosquats (e.g. `reqwests` -> `requests`).
   - **Live Event Terminal:** Streaming execution logs, receipts, and verification badges.
   - **1-Click Undo Button:** Instant rollback of any recorded transaction.

---

### Milestone 8: Full Undo Engine, Polish & Demo Attack Showcase
**Goal:** Complete transaction rollback engine, end-to-end polish, and scripted attack showcase.

#### Tasks:
1. **Comprehensive Undo Rollback:**
   - Reverse `AddToUserPath` via registry / PATH restore.
   - Reverse `InstallPackage` using `rollback_hint` (e.g. `winget uninstall`).
   - Delete artifacts downloaded by `DownloadArtifact`.
   - Clean up isolated virtual environments (`CreateIsolatedEnv`).
2. **Scripted Demo Attack Showcase:**
   - CLI: `orchestrator demo-attack`
   - GUI: "Simulate Attack" button demonstrating a malicious GitHub repository README attempting:
     - Typosquat package substitution (`reqwests` -> caught by Levenshtein radar).
     - Shell pipe execution (`curl ... | sh` -> caught by policy filter).
     - Private credential access (`~/.ssh/id_rsa` -> caught by policy filter).
     - Unapproved binary download (`evil-domain.ru` -> caught by domain allowlist).
   - Shows both CLI and GUI halting the attack in real-time with `[RED score=100/100]`.

---

## 🛡 Coding Standards (Enforced Across the Entire Project)

1. **Zero `.unwrap()` or `.expect()`** in non-test production code. Always use `?` and typed `thiserror` enums in `core` and `anyhow::Result` in `cli`/`gui`.
2. **Never invoke shell interpreters directly.** Do not pass strings to `cmd.exe /c` or `powershell -c`. All process executions must use direct binary invocation with explicit arguments via `OsProvider::run_command`.
3. **No destructive actions in tests.** Unit and integration tests must run against `MockOsProvider` and `MockAdapter`.
4. **Never hardcode API keys.** All secrets and endpoints are loaded from environment variables (`NIM_API_KEY`, `NIM_BASE_URL`, `NIM_MODEL`) or local `.env`.
5. **Always test compile before committing:**
   ```bash
   cargo test
   cargo clippy -- -D warnings
   ```
