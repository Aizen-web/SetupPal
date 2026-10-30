# Contributing to SetupPal

Thanks for helping improve SetupPal. This guide covers local setup, checks, and pull-request expectations.

## Prerequisites

- **Windows 10/11** with WebView2 (preinstalled on current Windows)
- **Rust** MSVC toolchain (`stable-x86_64-pc-windows-msvc`) and a linker (`link.exe` from Visual Studio Build Tools)
- **Node.js** (LTS) and npm
- Optional: a Nebius Token Factory API key for live chat/planning (see [README](README.md#provider-and-key-configuration))

## Run the CLI

```bash
cargo run -p installer_cli -- doctor
cargo run -p installer_cli -- plan "set me up for C++20 dev"
```

## Run the desktop app

```bash
npm --prefix crates/gui install
npm --prefix crates/gui run app         # tauri dev
npm --prefix crates/gui run build:app   # NSIS installer under target/release/bundle/nsis
```

`start-app.bat` runs the same `app` script. The backend binds a random `127.0.0.1` port; the window receives the address and bearer token from the Tauri shell.

## Checks before you open a PR

```powershell
powershell -File scripts/check.ps1
```

That runs workspace Rust tests, the frontend build, and the animation, key-migration, chat-formatter, run-log, plan-UI, and offline-assets checks. Keep it green.

Manual and release verification steps live in [docs/TESTING.md](docs/TESTING.md). UI conventions live in [docs/UI.md](docs/UI.md).

## Token and prompt hygiene (accurate, no magic cache claims)

Chat uses a fixed static system prompt so the instruction prefix stays stable across turns. Conversation history sent to the model is windowed and truncated so long threads do not grow without bound. Do not invent or document speculative “token cache” behavior that the code does not implement.

## Key hygiene

- Never commit API keys, `.env`, or credential dumps.
- Copy `.env.example` to a local `.env` for CLI/dev only; leave example values blank.
- In the desktop app, paste keys in **Accounts** (Windows Credential Manager). Do not paste keys into issues, PRs, docs, or chat logs.
- If a key appears in git history, rotate it; blanking the working tree file is not enough.

## Pull requests

- Keep changes focused: one concern per PR when practical.
- Describe what changed and how you verified it (`scripts/check.ps1`, relevant rows in [docs/TESTING.md](docs/TESTING.md)).
- Prefer real data and real errors in tests and UI—no placeholder or canned demo content.
- Match existing code style; do not reformat unrelated files.

Questions about product scope and safety behavior are answered in [README.md](README.md) and [SECURITY.md](SECURITY.md).
