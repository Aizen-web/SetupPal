# SetupPal testing guide

How to verify SetupPal before a PR or a public build: automated checks, prompt corpus runs, NSIS end-to-end scenarios, release gates, and known limitations.

## Automated checks (CI-safe)

No network and no API keys required:

```powershell
powershell -File scripts/check.ps1
```

This runs, in order:

1. `cargo test --workspace`
2. Frontend production build
3. Animation registry check
4. Key-migration check
5. Chat-formatter check
6. Run-log check
7. Plan UI check
8. Offline assets and CSP (`'self'`-only; no remote fonts)

Keep this green before calling a change release-ready.

### Prompt / context notes

The desktop chat path uses a fixed static system prompt and sends a bounded, truncated conversation history to the provider. Do not assert speculative session token-cache hit rates in tests or docs unless the code under test implements them.

## Prompt corpus (personas)

Interactive persona drives live against a debuggable Tauri webview. They chat and read plan/manual cards; they do **not** press Approve (nothing is installed).

Setup (from a machine with a provider key saved or in the environment):

```powershell
# In crates/gui (or via your usual tauri launch):
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9333'
$env:ORCHESTRATOR_STATE_LOG="$env:TEMP\setuppal-persona-state.jsonl"
npm --prefix crates/gui run app
```

Then:

```powershell
node scripts/persona-tests/personas.mjs gamer
node scripts/persona-tests/personas.mjs developer
node scripts/persona-tests/personas.mjs casual
```

Low-level CDP helper:

```powershell
node scripts/persona-tests/drive.mjs new
node scripts/persona-tests/drive.mjs say "install steam and discord"
node scripts/persona-tests/drive.mjs snap
```

**Corpus expectations (release bar):** cover gamer / developer / casual requests. For each request, the outcome should be one of: an exact set of install IDs, a Manual setup guide for a named item, a clarifying question, or an “already installed” answer. Deterministic pieces (resolver, scope, installability) should pass consistently. Model-dependent outcomes should never add an unrequested install step. Prefer repeating each request a few times across the three Nemotron models (Lightning / Super / Ultra) on Nebius.

Deterministic (key-free) corpus validation is part of `scripts/check.ps1`:

```powershell
node scripts/persona-tests/run-corpus.mjs --deterministic
```

Live CDP assertion runs (requires the app with remote debugging; never clicks Approve):

```powershell
node scripts/persona-tests/run-corpus.mjs --live --runs 1
```

Clean-VM before/after diffs use `scripts/vm-diff/` (`snapshot.ps1`, `compare.ps1`). See that folder’s README. Do not run destructive install/Rewind tests on a daily-driver account.

## Release gates

These gates describe what a release-candidate build should satisfy. Record results honestly.

1. **`scripts/check.ps1` is green** (unit and UI static checks).
2. **Prompt corpus** across the three personas, on both providers when keys are available, with zero runs that install something the user did not ask for.
3. **Clean-VM install and Rewind:** Windows 11 VM (or clean user account) with no prior SetupPal state. Install then Rewind representative packages (for example OBS, VLC, LMMS, Audacity, Python 3.12 + venv recipe, ripgrep, a TypeScript npm package in a chosen folder). Before/after diffs of package list, user/machine PATH, `%ProgramFiles%`, `%LOCALAPPDATA%\Programs`, Start Menu, `%USERPROFILE%\.venvs`, and the app folder should be empty. No leftover `winget.exe` / `msiexec.exe` / `unins*.exe` processes.
4. **Already-installed safety:** on a machine that already has Git, Node, and Python, ask for each plus broad “web dev” / “everyday apps” prompts. Nothing already present is reinstalled, upgraded, or added to History as a new install. Rewinding SetupPal changes leaves those pre-existing tools untouched.
5. **Cancel and concurrency:** start an install, cancel it, close the app, Rewind repeatedly. No leftover installer processes; one uninstall per rollback.
6. **Console clean:** no unhandled promise rejections and no CSP violations during persona and E2E runs.
7. **NSIS E2E table** below passes on an installed build.

### Clean-VM status for this release

**Clean-VM install and Rewind gates were not run for this release** when no dedicated Windows 11 VM (or equivalent clean account snapshot) was available. Treat gate 3 (and the install/Rewind portions of gate 4) as **not verified** until they are executed and recorded. Automated `scripts/check.ps1` and any dry-run / non-VM E2E rows that were exercised still apply.

## NSIS end-to-end checklist

Build and install a release package, then run against a clean Windows 11 user account (no Node/Vite/`.env` in that account):

```powershell
npm --prefix crates/gui run build:app
# Install SetupPal_<version>_x64-setup.exe from target/release/bundle/nsis
```

Record pass/fail and notes for every row. Repeat items 1–7 once per provider you claim to support.

### Setup

- Keys: **Accounts** → paste Nebius key → Save → field shows “Saved”. Never paste keys into docs, issues, or chat. Pick Lightning / Super / Ultra under **Settings → AI model**.
- Optional: Tavily for source-consensus lines; GitHub token for higher rate limits.
- Start from empty History, or note the latest entry so new runs are identifiable.

### Scenarios

| # | Scenario | Steps | Expected | Result | Notes |
|---|---|---|---|---|---|
| 1 | Recipe install | Ask "set me up for Rust". Approve the plan. | Plan comes from the Rust recipe (no model call for the plan). Steps run, `rustc --version` verifies, confetti only when every step passed. | | |
| 2 | LLM-planned install | Ask for something with no recipe, for example "jq and ripgrep". | A plan appears with real steps and per-step verdicts. Approved steps run and verify. | | |
| 3 | Yellow approval | Use a plan that has a Yellow step (for example a package with no upstream checksum). | The run halts on the Yellow step. Tick "Approve this step", press "Approve and continue": only that step is added and the run continues. | | |
| 4 | Failure and healer | Force a failing verify (for example install a tool, then temporarily rename its binary, or pick a package that does not put a command on PATH). | One remediation plan is proposed and safety-checked. The run ends with an honest failure message, not confetti. Red remediations never run. | | |
| 5 | Cancel | Start a winget install and press Esc (or Stop). | The run stops within 2 seconds, the step is marked cancelled, and later steps do not run. | | |
| 6 | Undo | Open **History** and press Rewind on the latest change. | The tool is gone (`winget list`, or the folder, is gone) and PATH is restored. The History row shows Rewind. | | |
| 7 | Manual guide | Ask "install Minecraft Bedrock". Then ask "install LMMS" and "install Steam". | Bedrock: a manual setup card with Microsoft Store steps and clickable alternatives; nothing is installed. LMMS and Steam: a plan card (both install silently through winget), never a manual card. | | |
| 8 | Greeting and vague request | Say "hi", then "set me up". | A short greeting, then a question asking what to install. No plan card yet. | | |
| 9 | npm project folder | Ask for an npm package such as "install typescript with npm". | The plan card shows a **Project folder** row. The Approve button stays disabled until a folder is chosen. After choosing a folder inside your profile, the package appears in `<folder>\node_modules`. | | |
| 10 | Single instance | Launch SetupPal a second time while it is running. | No second window. The existing window is focused. | | |
| 11 | Keychain | Save, then Remove, a key in Accounts. Run `cmdkey /list`. | A `SetupPal` entry exists after Save and is gone after Remove. `localStorage` holds no key-like values. Restarting the app keeps chat working without `.env`. | | |
| 12 | Security | From a terminal run `curl http://127.0.0.1:<port>/api/health` (port from the app log or `netstat`). Also run `curl -H "Origin: https://example.com" ...`. | HTTP 401 with an empty body. No `Access-Control-Allow-Origin` header for the foreign origin. No key or token appears in any log. | | |
| 13 | Offline UI | Disconnect from the network and launch the app. | Fonts and layout look identical. The console shows no CSP violations or failed font requests. | | |
| 14 | Model selector | Open Settings → AI model. Switch Lightning / Super / Ultra, restart the app. | Selection persists; health/`/api/model` match the choice; arbitrary IDs cannot be entered. | | |
| 15 | Intent fence tolerance | Repeat row 7 and row 1 a few times on Lightning (faster model sometimes drops the fence). | No raw `{"kind":...}` JSON ever shows in a reply. The manual card or plan still appears. Code blocks and other JSON in a reply stay untouched. | | |
| 16 | Dry run label | Send a plan with `dry_run: true` to `/api/execute` (or run `scripts/check.ps1`'s executor tests). | The run log and plan card say DRY RUN, every step line starts with `[DRY RUN]`, the summary says nothing was installed, and there is no confetti. History shows a "Dry run" chip with no Rewind. | | |
| 17 | Scan log once | Send several chat messages in one conversation with the Hacker run log open. | "detected N installed tools" appears once, and again only if the detected tools change or in a new conversation. | | |

### After the run

- Rewind or uninstall test packages.
- Note any step that needed manual intervention; treat unexpected interventions as bugs unless listed under [Known limitations](#known-limitations).

## Known limitations

Honest constraints of the current tree:

- The pinned Nebius model ID should be confirmed against a live Nebius account before relying on production chat quality.
- Faster Lightning runs may occasionally emit unfenced intent JSON; the UI should still avoid showing raw fence payloads, but behavior can vary—row 15 covers this.
- Credential-shaped values may exist in older git history of `.env.example`. Blanking the current file does not remove history; rotate any exposed keys.
- No full contrast / accessibility audit has been completed across every page and viewport.
- Not every registered mascot animation has been performance-profiled.
- Tavily is used by the Safety Engine for source-domain consensus only—not as general chat web search.
- OSV.dev coverage in this tree focuses on PyPI and npm. winget, cargo, Modrinth, and vendor downloads rely on allowlists, hashes, optional Tavily consensus, and metadata signals.
- `run_command` uses a fixed multi-minute timeout; very large installs (Visual Studio Build Tools, LLVM, and similar) may need longer per-step budgets.
- Minecraft inspection helpers are Windows-path specific and only inform chat facts.
- **Clean-VM install/Rewind** (release gate 3) may be unverified for a given cut—see [Clean-VM status for this release](#clean-vm-status-for-this-release).
