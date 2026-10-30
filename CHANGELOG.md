# Changelog

All notable changes to SetupPal are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-10-30

### Added

- **SetupPal** public release: an AI-assisted, zero-trust environment and package orchestrator for Windows.
- **AI chat** with streaming replies, plan proposals from recipes or a pinned LLM provider, and a **plan card** as the single source of truth for what will run.
- **Safety Engine** in Rust: closed action schema, traffic-light risk ratings (Green / Yellow / Red), source transparency, hash checks, OSV and optional Tavily consensus, and user-scope execution.
- **Two install gates:** unattended CLI install/verify, plus Safety Engine pass. Apps that fail either gate get a **Manual setup guide** instead of an install plan.
- **Desktop shell:** Tauri v2 app with local Axum API, per-launch bearer token, and **NSIS** installer packaging.
- **Providers:** Nebius (production) and NVIDIA NIM (testing), each pinned to one fixed model ID; keys via Accounts / OS keychain or local `.env` in development.
- **Rewind:** append-only transaction ledger with one-click undo of SetupPal-managed changes.
- **Pixel** mascot: flavor-only speech bubbles that never quote the user; Chill / Hacker modes and Settings appearance controls.
