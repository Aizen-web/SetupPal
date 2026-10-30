# SetupPal

**Say what you want on your PC. SetupPal plans it, checks it, runs only what you approve, verifies it, and can undo it.**

[![CI](https://github.com/Aizen-web/SetupPal/actions/workflows/ci.yml/badge.svg)](https://github.com/Aizen-web/SetupPal/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D6.svg)](https://github.com/Aizen-web/SetupPal/releases)

**Repository:** [github.com/Aizen-web/SetupPal](https://github.com/Aizen-web/SetupPal)
**License:** [MIT](LICENSE) (visible at the top of this repository)
**Hackathon track:** Best Apps and Agents — [Nebius × NVIDIA Global AI Hackathon](https://nebiusglobalaihackathon.devpost.com/)
**Built with:** NVIDIA Nemotron on [Nebius Token Factory](https://tokenfactory.nebius.com/) · optional Tavily for Safety Engine consensus

SetupPal is a zero-trust AI environment and package orchestrator for Windows. You describe a goal in plain English—developer toolchains, everyday apps, game launchers, music tools, mod setups—and SetupPal either builds a typed install **plan** (from a deterministic YAML recipe or a pinned AI provider) or gives you a **Manual setup guide** when automated install is not allowed.

Nothing the model suggests runs until it passes a deterministic Rust **Safety Engine**. Execution uses a closed action schema (no free-form shell), user-scope installs where possible, hash checks, post-install verification, and a one-click **Rewind** ledger.

## Why this exists

Installing software from a chat model is convenient and dangerous. Models invent package names, miss typosquats, and cannot be trusted with a shell. SetupPal separates **proposal** from **authority**: Nemotron on Nebius Token Factory turns a sentence into a typed plan, then Rust gates decide what may run. The user still approves Yellow steps. Red never runs.

That is a complete product experience—not a prompt demo: chat → plan card → Safety Engine → approve → verify → Rewind.

## How it uses Nebius and NVIDIA

| Piece | Detail |
| --- | --- |
| **Provider** | [Nebius Token Factory](https://tokenfactory.nebius.com/) OpenAI-compatible API (`https://api.tokenfactory.nebius.com/v1`) only |
| **Selectable models** | Nemotron 3.5 Lightning (`nvidia/Nemotron-3_5-Lightning`), **Nemotron 3 Super** default (`nvidia/nemotron-3-super-120b-a12b`), Nemotron 3 Ultra (`nvidia/Nemotron-3-Ultra-550b-a55b`) — catalog IDs only |
| **What the model does** | Streams chat replies and emits structured install intents / plans |
| **What the model never does** | Execute shell commands, bypass the Safety Engine, or override Red ratings |
| **Optional** | Tavily — Safety Engine source-domain consensus only (not general chat search) |

Pick a model in **Settings**. Default is Nemotron 3 Super. Choice is persisted across restarts; unknown IDs are rejected with a clear error (no silent fallback). Keys live in Windows Credential Manager (desktop) or `.env` (CLI / debug).

## Architecture (proposal ≠ authority)

```text
User goal (chat)
    │
    ▼
Nebius Token Factory ──► selected Nemotron model (default: Super)
    │                      (chat stream + setuppal-intent)
    ▼
Typed Plan card (source of truth)
    │
    ▼
Rust Safety Engine + installability gate
    │     Green / Yellow / Red
    ▼
User approval ──► closed Step adapters (winget, npm, pip, cargo, …)
    │
    ▼
Verify + JSONL ledger ──► one-click Rewind
```

The desktop shell is **Tauri v2** around a local Axum API (random `127.0.0.1` port, per-launch bearer token) and ships as an **NSIS** installer. A CLI shares the same `installer_core` crate.

## What SetupPal will install

SetupPal installs **anything that passes two gates**:

1. **Unattended CLI install** — it can be installed and verified from the command line without interaction (`winget` silent installs, `npm`, `pip`, `cargo`, Modrinth/Fabric CLI, and similar).
2. **Safety Engine** — every step gets a Green or approved Yellow rating. Red steps never run. See [SECURITY.md](SECURITY.md).

If a package is in winget’s community repository with a silent installer type, it qualifies—even if it has a GUI or asks you to sign in after install (Steam, Epic, Roblox, Discord, LMMS, and the like). Having a window is never a reason to refuse.

Anything else gets a **Manual setup guide**: step-by-step instructions plus similar things SetupPal *can* install. That includes Microsoft Store-only apps (for example Minecraft Bedrock), vendor download-page drivers, installers with no unattended mode, and anything the Safety Engine blocks. The gate in `crates/core/src/installability.rs` decides from manifests and policy—not the model’s opinion.

## Behavior bar

- Install **exactly** what the user asked for—nothing extra.
- The **plan card is the plan**; chat text must agree with it.
- Do **not** recommend, reinstall, or touch software that is already installed or unneeded.
- **Rewind** must leave no SetupPal-managed leftovers (packages, PATH entries, folders it created).

## Quick start for judges

### Option A — Download the Windows installer (fastest)

1. Open the latest [GitHub Release](https://github.com/Aizen-web/SetupPal/releases).
2. Download `SetupPal_*_x64-setup.exe` and the matching `*.sha256` checksum file.
3. Verify the checksum (PowerShell):

```powershell
Get-FileHash .\SetupPal_0.1.0_x64-setup.exe -Algorithm SHA256
# Compare to the hash in the .sha256 file from the same release.
```

4. Run the NSIS installer (current-user install). Launch SetupPal.
5. Open **Accounts**, paste a **Nebius Token Factory** API key, confirm it shows **Saved** (the value is never shown again).
6. In chat, try: `set me up for Python data science` — you should see a streaming reply powered by Nemotron, then a plan card with Safety Engine ratings. Approve only what you understand; use **Rewind** in History afterward.

A Nebius API key is required for live AI planning. SetupPal does not ship with a shared key.

### Option B — Build from source

Requires **Windows**, Node.js 22+, the Rust MSVC toolchain, and WebView2.

```bash
git clone https://github.com/Aizen-web/SetupPal.git
cd SetupPal
npm --prefix crates/gui install
npm --prefix crates/gui run app         # tauri dev
npm --prefix crates/gui run build:app   # NSIS under target/release/bundle/nsis
```

`start-app.bat` runs the same `app` script. Before opening a PR or cutting a build, run:

```powershell
powershell -File scripts/check.ps1
```

### CLI

```bash
cargo run -p installer_cli -- doctor
cargo run -p installer_cli -- plan "set me up for C++20 dev"
cargo run -p installer_cli -- plan "setup Rust + Node.js microservice environment"
cargo run -p installer_cli -- plan "set me up for C++20 dev" --json
```

AI planning needs `NEBIUS_API_KEY` (see below).

## Provider and key configuration

In the desktop app, open **Accounts** and paste your Nebius key. It is stored in Windows Credential Manager (service `SetupPal`) and is never shown again. Choose Lightning / Super / Ultra under **Settings → AI model**.

For CLI and local development, copy `.env.example` to `.env` and fill blanks locally. The app reads `.env` only in debug builds or with `SETUPPAL_DEV=1`. **Never** put real keys in `.env.example`, commit `.env`, or paste keys into docs.

```dotenv
# Optional; must be `nebius` or blank.
SETUPPAL_PROVIDER=

# lightning | super (default) | ultra — or an exact catalog API ID
SETUPPAL_MODEL=super

NEBIUS_BASE_URL=https://api.tokenfactory.nebius.com/v1
NEBIUS_API_KEY=

TAVILY_API_KEY=
```

- Inference is **Nebius Token Factory only**. NVIDIA NIM is not used.
- Users pick among three allowlisted Nemotron models in Settings; arbitrary model IDs are rejected.
- Chat and planning use the same selected model. The Safety Engine never reads the model choice.
- If a model is unavailable or fails, SetupPal shows a clear error and does not fall back to another model.
- Lookup order for keys: OS keychain, then process environment, then `.env` (developer fallback).

## What judges can verify

| Claim | How to check |
| --- | --- |
| MIT license | [LICENSE](LICENSE) at repository root |
| Nebius + Nemotron | Accounts → Nebius key Saved; Settings → Nemotron model; catalog IDs in `crates/core/src/config.rs` |
| Model never gets a shell | Closed `Step` enum in `crates/core/src/types.rs`; Red blocked in Safety Engine |
| Plan card is the plan | Chat must match the card; Approve runs only listed steps |
| Rewind works | History → Rewind after a successful install |
| CI is green | [GitHub Actions CI](https://github.com/Aizen-web/SetupPal/actions/workflows/ci.yml) runs `scripts/check.ps1` |
| Demo script | [docs/DEMO.md](docs/DEMO.md) (~3 minutes, Nebius/Nemotron called out) |
| Devpost copy | [SUBMISSION.md](SUBMISSION.md) |

## Known limitations (honest)

- **Windows only** (Tauri / WebView2 / NSIS / winget-focused adapters).
- Live AI needs a **BYOK** Nebius key—no shared judge key.
- Default CI is **key-free**; live persona corpus and clean-VM Rewind gates are documented in [docs/TESTING.md](docs/TESTING.md) and are not claimed as fully verified for every release cut.
- Long installs may time out; OSV coverage focuses on ecosystems the adapters use; no full accessibility audit yet.

## Workspace layout

| Crate | Role |
| --- | --- |
| `crates/core` (`installer_core`) | Domain logic: scan, recipes, LLM client, Safety Engine, adapters, executor, verifier, healer, state ledger |
| `crates/cli` (`installer_cli`) | Terminal UI (`clap`, `colored`, `indicatif`) |
| `crates/gui` | Desktop UI (vanilla JS + Vite) + Tauri v2 shell and NSIS packaging |

## Chat and Pixel

Replies stream as they arrive. Stop cancels the active reply and keeps any partial text; it does not bypass task approval. Chat shows only real model output and real errors—no placeholders.

**Pixel** is the only mascot. Pixel’s speech bubbles are flavor only: short, ambiguous, and fun. They never quote or paraphrase your messages. Facts live in the plan card, Task panel, terminal, and History.

## Docs

- [SUBMISSION.md](SUBMISSION.md) — Devpost-ready submission copy and checklist
- [SECURITY.md](SECURITY.md) — gates, traffic-light model, protection layers, technical appendix
- [CONTRIBUTING.md](CONTRIBUTING.md) — setup, checks, PR norms
- [docs/TESTING.md](docs/TESTING.md) — automated checks, corpus, NSIS E2E, limitations
- [docs/UI.md](docs/UI.md) — UI guide
- [docs/DEMO.md](docs/DEMO.md) — short presenter / recording script
- [CHANGELOG.md](CHANGELOG.md) — release notes
- [LICENSE](LICENSE) — MIT
