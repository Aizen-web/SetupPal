# Zero-Trust AI Environment & Package Orchestrator

An AI-powered installer and environment setup agent in Rust. State a goal in plain English (`"set up a fullstack Node & TypeScript web app"`, `"set me up for C++20 dev"`, `"install Rust toolchain"`, `"setup Python AI data science venv"`), and the orchestrator scans your host machine, checks deterministic YAML recipes first, or uses NVIDIA NIM with `schemars` JSON Schema enforcement to propose a strictly typed `Plan`.

Every step passes through a **deterministic Rust Safety Engine** before execution.

## Supported Software Scope

> **Important Scope Boundary**: SetupPal strictly installs software and environments that support **unattended Command-Line Interface (CLI) setup** (such as package managers like `winget`, `npm`, `pypi`, and `cargo`, programming languages, compilers, runtimes, web frameworks, SDKs, and developer utilities).
>
> **Not Supported**: Desktop applications requiring proprietary GUI launchers, interactive account logins, or DRM store wrappers (e.g., desktop games with launchers like Minecraft Bedrock, Steam, Epic) are **not supported**. These cannot be sandboxed, cryptographically audited, or installed headlessly through deterministic CLI automation.

## Workspace Layout
- `crates/core` (`installer_core`): All domain logic, traits, OS abstraction, system scanner, recipes, LLM client, safety engine, adapters, executor, verifier, healer, and state transaction log.
- `crates/cli` (`installer_cli`): Terminal app built on `installer_core` using `clap`, `colored`, and `indicatif`.
- `crates/gui` (`installer_gui` + vanilla JavaScript + Vite): Tauri v2 desktop app sharing `installer_core`.

## Quick Start (Milestone 1)

```bash
# 1. Run system scan & doctor check
cargo run -p installer_cli -- doctor

# 3. Generate a deterministic Recipe-backed Plan (C++20 dev)
cargo run -p installer_cli -- plan "set me up for C++20 dev"

# 4. Generate an NVIDIA NIM Plan (requires NIM_API_KEY in .env)
cargo run -p installer_cli -- plan "setup Rust + Node.js microservice environment"

# 5. Output raw JSON Plan + JSON Schema
cargo run -p installer_cli -- plan "set me up for C++20 dev" --json
```

## Nebius and Tavily configuration

Copy `.env.example` to `.env` and add keys locally. Never put real keys in `.env.example` or commit `.env`.

```dotenv
NEBIUS_API_KEY=
NEBIUS_BASE_URL=https://api.tokenfactory.nebius.com/v1
# Optional. If omitted, SetupPal loads the live model list and selects a Nemotron model.
NEBIUS_MODEL=
TAVILY_API_KEY=
```

Chat needs only one provider key: either `NEBIUS_API_KEY` or NVIDIA's `NIM_API_KEY` (also accepted as `NVIDIA_API_KEY`). When a Nebius key is configured, chat prefers Nebius; otherwise it uses NVIDIA. Nebius models are fetched by the Rust backend, and the default selection is chosen from the live list rather than a hard-coded Nebius model ID. The Accounts page can test an environment key and list Nebius models without saving the key in browser storage. Tavily remains an optional backend safety source verifier.

## Chat and appearance

Chat replies stream as they arrive. Requests are queued within each conversation, and Stop cancels the active reply while preserving any partial text; this does not change task execution or its approval requirements. Choose Pixel (playful), Mochi (shy and gentle), or Byte (dry and grumpy) from the chat header; each uses its own response voice. Settings includes Off, Subtle, and Lively options for the theme-aware ambient background.

The orchestrator **only installs tools that support unattended CLI setup** (web technologies, programming languages, compilers, runtimes, SDKs, package managers, and developer tools). It does not install desktop apps that require proprietary GUI launchers or interactive account logins.
