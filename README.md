# SetupPal: Zero-Trust AI Environment & Package Orchestrator

An AI-powered installer and environment setup agent in Rust. State a goal in plain English (`"set up a fullstack Node & TypeScript web app"`, `"set me up for C++20 dev"`, `"install Rust toolchain"`, `"set up Fabric mods for Minecraft Java"`), and SetupPal scans your machine, checks deterministic YAML recipes first, or asks the configured AI provider (with `schemars` JSON Schema enforcement) to propose a strictly typed `Plan`.

Every step passes through a **deterministic Rust Safety Engine** before execution.

The implementation roadmap, current status, and phased task list live in [FINISH_PLAN.md](FINISH_PLAN.md).

## What SetupPal Can Install

SetupPal installs **anything that passes two gates**:

1. **Unattended CLI install:** it can be installed and verified from the command line without user interaction (for example `winget` silent installs, `npm`, `pip`, `cargo`, or Modrinth/Fabric through its CLI installer).
2. **Safety Engine:** it passes the deterministic safety checks described in [SECURITY.md](SECURITY.md).

That covers programming languages, runtimes, compilers, SDKs, package managers, and developer tools, but also everyday apps and mod setups, as long as they meet both gates.

Anything else gets a **Manual setup guide** instead of an install plan. SetupPal explains that the app does not allow non-terminal setup, gives step-by-step instructions to do it yourself, and lists similar things it *can* install. This applies to apps that need proprietary GUI launchers, DRM activation, or interactive account logins (for example Steam, Epic, or Minecraft Bedrock).

## Workspace Layout

- `crates/core` (`installer_core`): All domain logic, traits, OS abstraction, system scanner, recipes, LLM client, safety engine, adapters, executor, verifier, healer, and state transaction log.
- `crates/cli` (`installer_cli`): Terminal app built on `installer_core` using `clap`, `colored`, and `indicatif`.
- `crates/gui` (`installer_gui` + vanilla JavaScript + Vite): The desktop app sharing `installer_core`. A real **Tauri v2** app (crates/gui/src-tauri) that embeds the local Axum API on a random 127.0.0.1 port, protected by a per-launch bearer token, and ships as an NSIS installer.

## Quick Start

### CLI

```bash
# 1. Run system scan & doctor check
cargo run -p installer_cli -- doctor

# 2. Generate a deterministic Recipe-backed Plan (C++20 dev)
cargo run -p installer_cli -- plan "set me up for C++20 dev"

# 3. Generate an AI-planned Plan (requires NEBIUS_API_KEY or NIM_API_KEY in .env)
cargo run -p installer_cli -- plan "setup Rust + Node.js microservice environment"

# 4. Output raw JSON Plan + JSON Schema
cargo run -p installer_cli -- plan "set me up for C++20 dev" --json
```

### Desktop app

Requires Node.js, the Rust MSVC toolchain, and the WebView2 runtime (preinstalled on Windows 10/11). The Tauri CLI is installed through npm.

```bash
npm --prefix crates/gui install
npm --prefix crates/gui run app         # tauri dev: starts Vite and opens one SetupPal window
npm --prefix crates/gui run build:app   # tauri build: NSIS installer in target/release/bundle/nsis
```

`start-app.bat` runs the same `app` script. There is no fixed port: the backend binds `127.0.0.1:0` and the window asks the shell for the address and token (`backend_info` command). Run `scripts/check.ps1` for the Rust tests, frontend build, and JS checks.
## Provider and Tavily configuration

In the desktop app, open **Accounts** and paste your keys: they are saved to Windows Credential Manager (service `SetupPal`) and are never shown again. For development and the CLI you can instead copy `.env.example` to `.env` and add keys locally (the app reads `.env` only in debug builds or with `SETUPPAL_DEV=1`). Never put real keys in `.env.example`, commit `.env`, or paste keys into docs.

```dotenv
# Which AI provider to use: nebius (production) or nim (testing).
# Defaults to nebius when NEBIUS_API_KEY is set.
SETUPPAL_PROVIDER=
NEBIUS_API_KEY=
NIM_API_KEY=
# Optional safety source verifier.
TAVILY_API_KEY=
```

- **Nebius** is the production provider and **NVIDIA NIM** is the testing provider. Each is pinned to one fixed model ID in code. There is no live model list and no model picker. (Provider pinning and `SETUPPAL_PROVIDER` are Phase 4 of [FINISH_PLAN.md](FINISH_PLAN.md).)
- `SETUPPAL_PROVIDER=nebius|nim` selects the provider explicitly. If it is unset, SetupPal uses Nebius when its key exists, otherwise NIM. `NVIDIA_API_KEY` is also accepted in place of `NIM_API_KEY`.
- Chat and the planner use the same provider and model.
- Tavily is an optional backend safety source verifier.
- Key lookup order: OS keychain, then process environment, then `.env` (developer fallback only). Saving a Tavily or GitHub key in Accounts applies to the next plan without a restart. Old browser-stored keys are moved to the keychain on first launch.

## Chat and appearance

Chat replies stream as they arrive. Requests are queued within each conversation, and Stop cancels the active reply while preserving any partial text; this does not change task execution or its approval requirements. Chat shows only real model output and real errors.

**Pixel** is SetupPal's only mascot. Pixel's speech bubbles are ambiguous and fun and never quote your messages. Settings includes Off, Subtle, and Lively options for the theme-aware ambient background.
