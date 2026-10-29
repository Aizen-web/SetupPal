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
- `crates/gui` (`installer_gui` + vanilla JavaScript + Vite): The desktop app sharing `installer_core`. The target shell is a real **Tauri v2** app that embeds the local API server and ships as an NSIS installer. This is being implemented per [FINISH_PLAN.md](FINISH_PLAN.md); until then the GUI runs as an Axum server plus a Vite dev server in an Edge app window.

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

The Tauri v2 build (one-command dev and build scripts) is tracked in [FINISH_PLAN.md](FINISH_PLAN.md). The current interim launcher is:

```bash
npm --prefix crates/gui install
npm --prefix crates/gui run app
```

## Provider and Tavily configuration

Copy `.env.example` to `.env` and add keys locally. Never put real keys in `.env.example`, commit `.env`, or paste keys into docs.

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
- In the finished Tauri app, keys are stored in the OS keychain; `.env` remains a developer fallback.

## Chat and appearance

Chat replies stream as they arrive. Requests are queued within each conversation, and Stop cancels the active reply while preserving any partial text; this does not change task execution or its approval requirements. Chat shows only real model output and real errors.

**Pixel** is SetupPal's only mascot. Pixel's speech bubbles are ambiguous and fun and never quote your messages. Settings includes Off, Subtle, and Lively options for the theme-aware ambient background.
