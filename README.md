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
- `crates/gui` (`installer_gui` + Svelte + TypeScript + Vite): Tauri v2 desktop app sharing 100% of `installer_core`.

## Quick Start (Milestone 1)

```bash
# 1. Run unit tests across the workspace
cargo test --workspace

# 2. Run system scan & doctor check
cargo run -p installer_cli -- doctor

# 3. Generate a deterministic Recipe-backed Plan (C++20 dev)
cargo run -p installer_cli -- plan "set me up for C++20 dev"

# 4. Generate an NVIDIA NIM Plan using offline deterministic mock mode (NIM_MOCK=1)
NIM_MOCK=1 cargo run -p installer_cli -- plan "setup Rust + Node.js microservice environment"

# 5. Output raw JSON Plan + JSON Schema
NIM_MOCK=1 cargo run -p installer_cli -- plan "set me up for C++20 dev" --json
```
