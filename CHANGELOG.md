# Changelog

All notable changes to SetupPal are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.1] — 2026-10-30

Hackathon build. Chat shows the reply and the plan card; model reasoning stays behind a Thinking disclosure. The composer model picker can switch Lightning, Super, and Ultra.

### Added

- **Hackathon submission pack:** judge-facing README (Nebius Token Factory + NVIDIA Nemotron), [SUBMISSION.md](SUBMISSION.md) Devpost copy, demo recording guide in [docs/DEMO.md](docs/DEMO.md), and GitHub Release publishing (NSIS + SHA-256) on `v*` tags.
- **SetupPal** public release: an AI-assisted, zero-trust environment and package orchestrator for Windows.
- **AI chat** with streaming replies, plan proposals from recipes or a pinned LLM provider, and a **plan card** as the single source of truth for what will run.
- **Safety Engine** in Rust: closed action schema, traffic-light risk ratings (Green / Yellow / Red), source transparency, hash checks, OSV and optional Tavily consensus, and user-scope execution.
- **Two install gates:** unattended CLI install/verify, plus Safety Engine pass. Apps that fail either gate get a **Manual setup guide** instead of an install plan.
- **Desktop shell:** Tauri v2 app with local Axum API, per-launch bearer token, and **NSIS** installer packaging.
- **Providers:** Nebius Token Factory only, with a Settings picker for three catalog Nemotron models (Lightning default, Super, Ultra), also switchable from the chat composer; keys via Accounts / OS keychain or local `.env` in development.
- **Rewind:** append-only transaction ledger with one-click undo of SetupPal-managed changes.
- **Pixel** mascot: flavor-only speech bubbles that never quote the user; Chill / Hacker modes and Settings appearance controls.
