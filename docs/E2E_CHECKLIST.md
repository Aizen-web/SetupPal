# SetupPal manual end-to-end checklist

Run this against the installed NSIS build (`npm --prefix crates/gui run build:app`, then install
`SetupPal_<version>_x64-setup.exe` on a clean Windows 11 user account with no Node, Vite, or `.env`).
Record pass or fail plus notes for every row in the pull request. Repeat items 1 to 7 once per provider.

Automated checks come first and are CI-safe (no network, no keys):

```powershell
powershell -File scripts/check.ps1
```

That runs `cargo test --workspace`, the frontend build, and the animation, key-migration, chat-formatter,
and offline-assets (no remote fonts, local-only CSP) checks.

## Setup

- Keys: open **Accounts**, paste the provider key, press Save, and confirm the field shows "Saved".
  Never paste a key into a doc, issue, or chat. Use the provider you are testing:
  - Nebius (production): `SETUPPAL_PROVIDER=nebius`, or leave it unset when only the Nebius key is saved.
  - NVIDIA NIM (testing): `SETUPPAL_PROVIDER=nim`.
- Optional: save a Tavily key to see source-consensus lines in the Why? panels, and a GitHub token for
  higher rate limits.
- Start from a clean slate: History empty, or note the latest entry so you can tell new runs apart.

## Checklist

| # | Scenario | Steps | Expected | Result | Notes |
|---|---|---|---|---|---|
| 1 | Recipe install | Ask "set me up for Rust". Approve the plan. | Plan comes from the Rust recipe (no model call for the plan). Steps run, `rustc --version` verifies, confetti only when every step passed. | | |
| 2 | LLM-planned install | Ask for something with no recipe, for example "jq and ripgrep". | A plan appears with real steps and per-step verdicts. Approved steps run and verify. | | |
| 3 | Yellow approval | Use a plan that has a Yellow step (for example a package with no upstream checksum). | The run halts on the Yellow step. Tick "Approve this step", press "Approve and continue": only that step is added and the run continues. | | |
| 4 | Failure and healer | Force a failing verify (for example install a tool, then temporarily rename its binary, or pick a package that does not put a command on PATH). | One remediation plan is proposed and safety-checked. The run ends with an honest failure message, not confetti. Red remediations never run. | | |
| 5 | Cancel | Start a winget install and press Esc (or Stop). | The run stops within 2 seconds, the step is marked cancelled, and later steps do not run. | | |
| 6 | Undo | Open **History** and press Rewind on the latest change. | The tool is gone (`winget list`, or the folder, is gone) and PATH is restored. The History row shows Rewind. | | |
| 7 | Manual guide | Ask "install Steam". | A manual setup card appears with step-by-step instructions and clickable alternatives. Nothing is installed. | | |
| 8 | Greeting and vague request | Say "hi", then "set me up". | A short greeting, then a question asking what to install. No plan card yet. | | |
| 9 | npm project folder | Ask for an npm package such as "install typescript with npm". | The plan card shows a **Project folder** row. The Approve button stays disabled until a folder is chosen. After choosing a folder inside your profile, the package appears in `<folder>\node_modules`. | | |
| 10 | Single instance | Launch SetupPal a second time while it is running. | No second window. The existing window is focused. | | |
| 11 | Keychain | Save, then Remove, a key in Accounts. Run `cmdkey /list`. | A `SetupPal` entry exists after Save and is gone after Remove. `localStorage` holds no key-like values. Restarting the app keeps chat working without `.env`. | | |
| 12 | Security | From a terminal run `curl http://127.0.0.1:<port>/api/health` (port from the app log or `netstat`). Also run `curl -H "Origin: https://example.com" ...`. | HTTP 401 with an empty body. No `Access-Control-Allow-Origin` header for the foreign origin. No key or token appears in any log. | | |
| 13 | Offline UI | Disconnect from the network and launch the app. | Fonts and layout look identical. The console shows no CSP violations or failed font requests. | | |
| 14 | Provider pin | Open Accounts and Settings. | The model shown is the pinned model for the selected provider; there is no model picker. | | |
| 15 | Intent fence tolerance | Repeat row 7 and row 1 a few times with NIM (the small model sometimes drops the fence). | No raw `{"kind":...}` JSON ever shows in a reply. The manual card or plan still appears. Code blocks and other JSON in a reply stay untouched. | | |
| 16 | Dry run label | Send a plan with `dry_run: true` to `/api/execute` (or run `scripts/check.ps1`'s executor tests). | The run log and plan card say DRY RUN, every step line starts with `[DRY RUN]`, the summary says nothing was installed, and there is no confetti. History shows a "Dry run" chip with no Rewind. | | |
| 17 | Scan log once | Send several chat messages in one conversation with the Hacker run log open. | "detected N installed tools" appears once, and again only if the detected tools change or in a new conversation. | | |

## After the run

- Delete any test packages you installed, or rewind them from History.
- Note any step that needed manual intervention; each one is a bug unless it is listed in
  `FINISH_PLAN.md` Appendix A.
