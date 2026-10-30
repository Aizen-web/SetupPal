# SetupPal

**Say what you want on your PC. SetupPal plans it, checks it, runs only what you approve, verifies it, and can undo it.**

SetupPal is a zero-trust AI environment and package orchestrator for Windows. You describe a goal in plain English—developer toolchains, everyday apps, game launchers, music tools, mod setups—and SetupPal either builds a typed install **plan** (from a deterministic YAML recipe or a pinned AI provider) or gives you a **Manual setup guide** when automated install is not allowed.

Nothing the model suggests runs until it passes a deterministic Rust **Safety Engine**. Execution uses a closed action schema (no free-form shell), user-scope installs where possible, hash checks, post-install verification, and a one-click **Rewind** ledger.

The desktop app is a real **Tauri v2** window around a local Axum API (random `127.0.0.1` port, per-launch bearer token) and ships as an **NSIS** installer. A CLI shares the same `installer_core` crate.

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

## Quick start

### CLI

```bash
cargo run -p installer_cli -- doctor
cargo run -p installer_cli -- plan "set me up for C++20 dev"
cargo run -p installer_cli -- plan "setup Rust + Node.js microservice environment"
cargo run -p installer_cli -- plan "set me up for C++20 dev" --json
```

AI planning needs `NEBIUS_API_KEY` or `NIM_API_KEY` (see below).

### Desktop app

Requires Node.js, the Rust MSVC toolchain, and WebView2.

```bash
npm --prefix crates/gui install
npm --prefix crates/gui run app         # tauri dev
npm --prefix crates/gui run build:app   # NSIS under target/release/bundle/nsis
```

`start-app.bat` runs the same `app` script. Before opening a PR or cutting a build, run:

```powershell
powershell -File scripts/check.ps1
```

## Provider and key configuration

In the desktop app, open **Accounts** and paste keys. They are stored in Windows Credential Manager (service `SetupPal`) and are never shown again.

For CLI and local development, copy `.env.example` to `.env` and fill blanks locally. The app reads `.env` only in debug builds or with `SETUPPAL_DEV=1`. **Never** put real keys in `.env.example`, commit `.env`, or paste keys into docs.

```dotenv
# Which AI provider to use: nebius (production) or nim (testing).
# Defaults to nebius when NEBIUS_API_KEY is set.
SETUPPAL_PROVIDER=

NEBIUS_BASE_URL=https://api.tokenfactory.nebius.com/v1
NEBIUS_API_KEY=

NIM_BASE_URL=https://integrate.api.nvidia.com/v1
NIM_API_KEY=
# NVIDIA_API_KEY=

TAVILY_API_KEY=
```

- **Nebius** is production; **NVIDIA NIM** is for testing. Each provider is pinned to one fixed model ID in code—no model picker and no live model list.
- `SETUPPAL_PROVIDER=nebius|nim` selects explicitly. If unset, SetupPal uses Nebius when its key exists, otherwise NIM. `NVIDIA_API_KEY` is accepted in place of `NIM_API_KEY`.
- Chat and planning use the same provider and model. Tavily is an optional Safety Engine source verifier.
- Lookup order: OS keychain, then process environment, then `.env` (developer fallback).

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

- [SECURITY.md](SECURITY.md) — gates, traffic-light model, protection layers, technical appendix
- [CONTRIBUTING.md](CONTRIBUTING.md) — setup, checks, PR norms
- [docs/TESTING.md](docs/TESTING.md) — automated checks, corpus, NSIS E2E, limitations
- [docs/UI.md](docs/UI.md) — UI guide (v0.3)
- [docs/DEMO.md](docs/DEMO.md) — short presenter script
- [CHANGELOG.md](CHANGELOG.md) — release notes
- [LICENSE](LICENSE) — MIT
