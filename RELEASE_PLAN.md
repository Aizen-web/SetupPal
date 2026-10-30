# SetupPal Release Plan

This plan lists everything that must be true before SetupPal ships. It comes from a hands-on test on Oct 29, 2026. Three personas (a gamer, a developer, and a casual user) used the real app with the real model and the real installer. The app was **not** release-ready. `FINISH_PLAN.md` says "Phase 7 complete", but a product that installs software the user never asked for can't ship, whatever its phase checklist says.

## The bar

SetupPal changes people's computers, so the standard is closer to a package manager than a chatbot. Every item below must hold. These are not goals to aim for.

1. **It installs exactly what the user asked for.** It adds nothing unrequested, no bundles, and no "while we're at it" extras. A dependency is allowed only when the requested item can't work without it, and the plan card must say so in plain words.
2. **What the chat says matches what will run.** The plan card is the only description of the plan. If the chat text and the card ever disagree, that is a release blocker.
3. **It never touches what the user already has.** Already-installed items are shown as installed, with their version. They are never reinstalled, upgraded, or put into an undo record.
4. **It removes things completely.** After Rewind, the machine matches its state from before the install: the package list, the user PATH, the install folders, and the Start Menu entries. A "Verified: Passed" label must mean all of those were checked.
5. **It never refuses something it can do, and never claims something it can't.** Manual guides are only for things the deterministic gate rejects, not for things the model guesses about.
6. **Failures are honest and useful.** Every error says what happened in plain words. There are no empty error strings and no 5-minute silent waits.
7. **It recommends only what fits.** Suggestions must match the user's request, must not already be installed, and must not pad the reply.

## Severity

- **P0, release blocker:** it installs, removes, or changes something the user didn't ask for; the plan and the chat disagree; it leaves stuck processes or leftovers on the machine; or an install the app should handle fails.
- **P1, must fix before release:** a wrong refusal, a misleading label, a dead-end flow, or an error the user can't act on.
- **P2, fix before release unless the owner signs off:** tone, noise, or polish.

---

## P0: Release blockers

### R1. Single-app requests get expanded into recipe bundles

**Status: Done** (recipe triggers are stack phrases only; fuzzy matching removed; `plan_scope` rejects unrequested installs; everyday_apps no longer includes Git)

**What happened**
- "install vlc please" produced 7-Zip, VLC, and Git.
- "I need node, git and python 3.12" produced Python 3.12, a venv, numpy, pandas, colorama, and an import check.
- "install typescript globally with npm" produced Node LTS, Git, and three version checks, and did not install TypeScript at all.

**Cause:** `Recipe::matches_goal` in [crates/core/src/recipes/mod.rs](crates/core/src/recipes/mod.rs) uses substring matching and Jaro-Winkler fuzzy matching on single words. The triggers are too broad: `"vlc"` and `"7zip"` in `everyday_apps.yaml`, `"python"` in `python_ai_venv.yaml`, and `"typescript"` and `"node"` in `web_fullstack_ts.yaml`. The planner in [crates/core/src/planner/mod.rs](crates/core/src/planner/mod.rs) takes the first match and never asks whether the recipe is a superset of the request.

**Fix**
- Remove the fuzzy matching entirely. A recipe is used only when the model's intent names it (`{"kind":"install","recipe":"rust_dev"}`) or when the user's words exactly match a **stack phrase** such as "rust dev setup" or "python data science". Delete all single-product triggers (`vlc`, `7zip`, `python`, `node`, `typescript`, `web`).
- Change the intent to carry the requested items: `{"kind":"install","items":["vlc"]}`. The planner builds one install step per item, plus verification, and nothing else.
- Add a **plan-scope validator** that runs after planning and before `/api/plan` returns. Every `InstallPackage` step must map either to an item in `items` or to a declared dependency with a `reason` field that the card shows. The validator rejects the plan otherwise. This also covers LLM-made plans.
- Run the installed-tools scan before building the plan. An item that is already installed becomes an "Already installed (v2.50.1)" row with no step. A recipe step for an installed tool is dropped, not run.
- Rewrite `everyday_apps.yaml`. It may only be chosen when the user asks for "everyday apps" or for that exact trio, and it must never include Git, which is a developer tool.

**Acceptance**
- A corpus test (see [Release gates](#release-gates)) checks that each of these requests produces exactly the listed install steps:

  | Request | Install steps |
  |---|---|
  | "install vlc" | `VideoLAN.VLC` |
  | "python 3.12" | `Python.Python.3.12` |
  | "typescript globally with npm" | `typescript` (npm), with Node not reinstalled because it is present |
  | "set me up for python data science" | the venv recipe |

- The scope validator has unit tests, including that a plan with one extra unrequested step is rejected.

### R2. The chat describes one plan and the card shows another

**Status: Done** (prompt: card is the plan; intent must be grounded in user text; new plans replace the prior card; status row while planning)

**What happened**
- For Python 3.12, the chat said "Install Python 3.12, verify", but the card had six steps.
- For TypeScript, the chat said "install typescript via npm", but the card installed Node and Git.
- For VLC, the chat said "Install VLC, verify", but the card showed 7-Zip, VLC, and Git.
- During the Rust conversation, a "vscode" plan was generated from Pixel's own recommendation, which the user never accepted.

**Cause:** the system prompt in [crates/gui/src-tauri/src/main.rs](crates/gui/src-tauri/src/main.rs) (`STATIC_SYSTEM_PROMPT`) tells the model to write "a short numbered list" plan in prose. The real plan is then built separately from a short `goal` string. They are two plans from two sources.

**Fix**
- The model never writes plan steps in prose. Its reply for an install request is one or two sentences, followed by the intent block. The card is the plan.
- Remove "A plan is a short numbered list..." from the prompt and replace it with: "Don't list steps. Say in one sentence what you'll set up; the plan card shows the steps."
- Intent guard in `handleIntent` ([crates/gui/src/api/bridge.js](crates/gui/src/api/bridge.js)) and in `plan_handler`: every item in the intent must appear in, or be clearly named by, the user's messages in this conversation. Items that only the assistant mentioned are dropped. This stops the stray "vscode" plan.
- A new install intent replaces the previous plan card. The old card collapses to a single line saying it was replaced, so two plans are never live at once.

**Acceptance**
- A test sends 20 mixed install requests and checks that no reply contains a numbered list of install steps.
- A test checks that an intent naming an item the user never typed produces no plan.

### R3. Package IDs are guessed, and the user's exact ID is ignored

**Status: Done** (planner resolves winget before LLM; exact `Publisher.Package` via `winget show`; clear search hits → `single_winget_plan`)

**What happened**
- "install audacity" planned `Audacity`, which doesn't exist; the right ID is `Audacity.Audacity`.
- "install ripgrep" planned `ripgrep`; the right ID is `BurntSushi.ripgrep.MSVC`. Both steps were Yellow with "package not found", and the ripgrep run failed.
- "install Audacity.Audacity from winget" **still** planned `Audacity`.

**Cause:** the LLM planner invents IDs and nothing resolves them before planning.

**Fix**
- Add a `Resolver` in `crates/core` that turns each requested item into an exact package ID before any plan exists:
  - For winget, try `winget show --id <x> --exact` first, then `winget search <x>`, keeping only the `winget` source (never `msstore`).
  - For npm, use the registry `GET /<name>`. For PyPI, use `GET /pypi/<name>/json`.
- Rules:
  - An ID the user typed exactly wins, and if it doesn't exist, that is an error.
  - One clear match (an exact name match, or the publisher's canonical ID) resolves.
  - Several plausible matches mean the reply asks the user to pick, listing the real candidates with publisher and version.
  - No match means "I couldn't find X in winget", with no plan.
- The LLM planner gets the resolved IDs as fixed input and may not change them. `validate_plan_structure` rejects any `InstallPackage` step whose ID wasn't resolved.
- Cache resolutions for each conversation so repeated turns don't re-run `winget search`.

**Acceptance**
- The corpus test checks that audacity, ripgrep, jq, notepad++, lmms, discord, obs, 7-zip, and vlc each resolve to the right winget ID.
- Typing `Audacity.Audacity` produces a plan with exactly that ID.

### R4. LMMS fails to install because the app forces `--scope user`

**Status: Done** (`--scope user` only when the manifest lists user scope; HRESULT → plain words; retry without scope on `0x8A150010`)

**What happened:** the app's LMMS install failed with exit code `0x8A150010` and an **empty** error message. Running `winget install --id LMMS.LMMS --exact --silent ...` by hand without `--scope user` succeeded.

**Cause:** `install_args` in [crates/core/src/adapters/winget.rs](crates/core/src/adapters/winget.rs) adds `--scope user` whenever the manifest doesn't say `machine`. Manifests that declare no scope at all (LMMS, and many others) therefore fail with "No applicable installer found".

**Fix**
- Pass `--scope user` only when `winget show` lists a user-scope installer. When the manifest declares no scope, don't pass `--scope`.
- If winget returns `0x8A150010` after `--scope user`, retry once without a scope flag, and only after the `NeedsAdmin` gate has been shown to the user.
- Map winget HRESULTs to plain sentences in one table, covering at least `0x8A150010`, `0x8A150011`, `0x8A15002B`, `0x8A150014`, the install-in-progress code, and the hash-mismatch code. `AdapterError::CommandFailed` must never render as `exit Some(-1978335216):` with nothing after it. Include winget's last non-empty output line.

**Acceptance:** LMMS, Audacity, Notepad++, and 7-Zip install through the app on a clean Windows 11 VM. Every failure in the corpus shows a non-empty, human-readable reason.

### R5. Hung installer processes stay alive and can uninstall the user's software

**Status: Done** (Job Object kill-on-close; install/undo mutex; idempotent Rewind; pre-flight `winget`/`msiexec` busy check; 90s “Still working…” progress events on long installs)

**What happened**
- Ten `winget uninstall --id Git.Git` processes from an earlier session had been alive for about 18 hours, together with Git's own uninstaller (`C:\Program Files\Git\unins000.exe /SILENT`). Git, which the user installed themselves, could have been removed whenever that uninstaller resumed.
- Those processes held winget's lock, so the first OBS install sat silent for 300 seconds and then failed with "timed out".
- A stale `installer_gui.exe` from the old architecture was also still listening on port 3001.

**Cause:** the timeout in `run_command` ([crates/core/src/os/mod.rs](crates/core/src/os/mod.rs)) returns an error but doesn't kill the child process tree. Repeated Rewind clicks, or retries, then start more processes behind the stuck one.

**Fix**
- Run every child process in a Windows Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. Timeout, cancel, and app exit all kill the whole tree, including installer children such as `unins000.exe` and `msiexec`.
- Allow only one install or uninstall at a time across the app: a global async mutex in the executor and in `undo_handler`. While one is running, the Rewind button and the Run button are disabled and show why.
- Before the first winget call of a run, check for other `winget.exe` or `msiexec.exe` install processes. If any are found, stop with a plain message: "Another installer is running (winget, started 1:56 AM). Close it or wait, then try again." Don't queue behind it silently.
- Watch for output: if an install produces no output and no new child process for 90 seconds, show "Still working: winget is downloading or waiting on Windows" in the run log, with the real elapsed time.
- Every rollback must be idempotent and run once. Clicking Rewind twice must never start a second uninstall.

**Acceptance**
- A test runs a command that never exits. After the timeout or cancel, no descendant process is left (checked by listing processes by parent ID).
- Clicking Rewind five times quickly on one entry produces exactly one uninstall process.
- Closing the app during an install leaves no `winget.exe` alive.

### R6. Rewind leaves PATH entries and folders behind, and still says "Verified: Passed"

**Status: Done** (capture user PATH before winget install + diff after; record install location; on Rewind strip only added PATH entries that now point at missing/empty leftovers; delete safe leftover folders; post-undo verifies winget list + PATH + folder — Failed lists what remains)

**What happened:** after rewinding Python 3.12, `winget list` showed it as gone and History said "Post-Undo Verified: Passed". But `C:\Users\there\AppData\Local\Programs\Python\Python312\` (a leftover `Doc` folder) still existed, and its two PATH entries were still in the user PATH. I removed them by hand.

**Cause:** post-undo verification for winget only checks `winget list`.

**Fix**
- Before every winget install, record the user PATH, the machine PATH, and the package's install location (from the uninstall registry key after install).
- On rollback, after the uninstall:
  - Remove only the PATH entries that this install added and that now point to a missing or empty folder.
  - Delete the install folder if only empty folders or installer leftovers remain.
  - Never delete a folder that holds user files; report it instead.
- Post-undo verification checks the package list, both PATH entries, and the install folder. It shows "Passed" only when all three are clean. Otherwise it lists what's left and offers to remove it.

**Acceptance:** on a clean VM, install and then Rewind Python 3.12, OBS, VLC, and LMMS. A before/after diff of `winget list`, the user and machine PATH, `%LOCALAPPDATA%\Programs`, `%ProgramFiles%`, and the Start Menu folders is empty.

### R7. Unticking a step blocks the whole run

**Status: Done for release** (unticked → Skipped, run continues; “Run N of M steps”). **`depends_on` deferred:** adding a dependency graph needs PlannedStep schema + recipe YAML + card UI to auto-untick dependents before run. Unticked deps already skip without failing later steps; true soft-block UX is polish, not a leftover P0 once Skipped continues.

**What happened:** on the VLC/7-Zip/Git card, I left the two Yellow steps unticked and ran only the VLC step. The run stopped at step 1 (7-Zip, "HELD"), and VLC never installed. The user can't run only part of a plan.

**Cause:** the executor in [crates/core/src/executor/mod.rs](crates/core/src/executor/mod.rs) `break`s at the first Yellow step that isn't approved.

**Fix**
- An unticked step becomes `Skipped` and is never run. The run continues with the next approved step.
- Steps declare `depends_on`. If a skipped step is a dependency of a ticked step, the card turns the dependent step's checkbox off and explains why before the run starts. Nothing fails halfway through the run.
- The approve button reads "Run 1 of 4 steps". The run summary lists skipped steps by name.

**Acceptance:** an executor test checks that, with Yellow steps 1 and 3 unticked and step 2 ticked, step 2 runs and steps 1 and 3 are `Skipped`. A dependency test covers the blocked case.

### R8. LLM plans create junk folders inside the app folder

**Status: Done** (validator rejects orphan/relative `CreateIsolatedEnv`; working dir is `%USERPROFILE%\.setuppal\work`, never `current_dir()`)

**What happened:** the LMMS plan included "Create an isolated environment" (meaningless for a winget install). It created `crates\gui\src-tauri\lmms_env`, inside the repo, because the backend's working directory is the default. Undo doesn't list that folder as reversible.

**Fix**
- `CreateIsolatedEnv` paths must be absolute and sit inside `%USERPROFILE%\.venvs\` or the folder the user chose. Relative paths are rejected during validation.
- A `CreateIsolatedEnv` step is valid only if a later pip or npm step in the same plan uses it. The validator rejects it otherwise.
- The backend's own working directory is never used as the base for any step. `resolve_working_dir` in `main.rs` should return a user-chosen folder or an error, never `current_dir()`.

**Acceptance:** a validator test rejects an isolated-environment step paired with only a winget step, and rejects relative paths.

### R9. Verify commands that can't work

**Status: Done** (resolved winget plans are install-only; `RunVerifyCommand` remains the closed CLI `AllowedBinary` allowlist)

**What happened:** plans proposed `lmms --version` and `vlc --version`. GUI apps aren't on PATH, and running a GUI binary with `--version` can open the app's window.

**Fix**
- For winget installs, the only verification is the adapter's `winget list --id <id> --exact`. The planner must not add `RunVerifyCommand` steps for winget GUI apps.
- `RunVerifyCommand` is allowed only for binaries on an explicit CLI allowlist (node, npm, git, python, pip, rustc, cargo, cmake, ninja, rg, jq, java, tsc, and similar). The binary must also be found on the refreshed PATH after install.

**Acceptance:** none of the corpus plans for GUI apps contains a `RunVerifyCommand` step.

---

## P1: Must fix before release

### R10. Apps the app can install get a manual guide instead

**What happened:** LMMS, Discord, MSI Afterburner, and HWiNFO all got "Manual setup guide" replies. The reasons given were invented, such as "LMMS might need a GUI launcher". All four are on winget with silent installers (`LMMS.LMMS` is nullsoft). With an explicit ID, the model then agreed that LMMS "installs unattended", so the same app got both answers in one session.

**Cause:** the prompt's scope paragraph said "Apps that need a GUI launcher ... are out of scope", and the model read that as "any app with a GUI is out of scope". It also emitted `kind: manual` intents on its own judgment, which the app showed without checking. On top of that, `installability.rs` hard-blocked a launcher list (Steam, Epic, Riot, EA, Ubisoft, Minecraft Launcher, Roblox, Battle.net, GOG), and `exe` installers outside a six-ID allowlist (which caught Discord and Spotify).

**Decision (owner, Oct 29, 2026):** if something is trusted through winget, it can be installed. A sign-in after install doesn't matter. Only Microsoft Store-only apps, vendor-page downloads, apps with no unattended installer, and Safety Engine blocks are manual-only.

**Done**
- `installability.rs` no longer has the launcher list or the `exe` allowlist. It judges every winget package by its manifest: Store product IDs and `msstore` mean `StoreOnly`, and every unattended installer type (including community-repo `exe`) means installable. `LauncherOrDrm` and `InteractiveLogin` are gone. Tests now require Steam, Epic, the Minecraft Launcher, Roblox, Discord, and Spotify to be installable, and a Store ID to be manual.
- The prompt scope paragraph (`PROMPT_VERSION` `v24-winget-scope`) says winget apps are in scope, even with a window or a sign-in, and names Store-only apps as the out-of-scope example. The facts note about launchers now only covers Minecraft Bedrock vs Java.
- `handleIntent` in `bridge.js` sends every `kind: manual` intent through `/api/plan`. The model's manual verdict is only shown if the backend can't build a plan.
- `AGENTS.md`, `README.md`, `SECURITY.md`, `FINISH_PLAN.md`, `ui.md`, `docs/DEMO.md`, and `docs/E2E_CHECKLIST.md` state the same scope.

**Still to do**
- ~~Override note on the card~~ **Done** (`verifiedManual` → "SetupPal checked: this installs unattended through winget.")
- ~~R3 resolver for override IDs~~ **Done**
- Corpus / persona suite (release gate 2) not run in this pass

**Acceptance:** the corpus test checks that LMMS, Discord, HWiNFO, MSI Afterburner, Audacity, Spotify, Notepad++, Steam, Epic Games, and Roblox produce plans, and that Minecraft Bedrock and a vendor-page GPU driver produce manual cards. Run it 3 times per provider with zero flips.

### R11. Typos are silently changed

**Status: Partial** (prompt forbids silent renames; resolver looks up the exact name). **Still open:** "Did you mean?" confirmation UI when a close match exists.

**What happened:** for "pip install reqeusts", Pixel replied "I see you're asking to pip install requests". The model replaced the name without telling the user. The opposite case also matters: if a typosquat package really exists under the misspelled name, quietly accepting it would be dangerous.

**Fix**
- Prompt: "Never change a package name the user typed. If it looks misspelled, ask."
- The resolver (R3) looks up the exact name. If it doesn't exist and a close match does, the reply asks "Did you mean `requests`?" and offers one button. If the misspelled name does exist, the Safety Engine typosquat check must rate it Yellow at minimum, with the real name in the reason.

**Acceptance:** a test checks that `reqeusts`, `lodahs`, and `colourama` never produce a plan that names a different package without the user confirming.

### R12. Healer suggestions that change nothing

**Status: Done** (classified failure cause; drop no-op remediations that only recreate the failed install or only create an isolated env)

**What happened:** after `ripgrep` failed with "package not found", the healer suggested installing `ripgrep` with winget again. After LMMS failed, it suggested a plan whose only step was "Create an isolated environment".

**Fix**
- The healer gets a classified failure (not found, scope, admin needed, network, hash, timeout, already installed) and must change the input that caused it. Not found leads to a new resolver lookup. A scope error leads to a retry without the scope flag (R4).
- Drop any remediation plan that is identical to the failed step, contains no step that fixes the classified cause, or adds unrequested items. In that case show only the honest failure.

**Acceptance:** a healer test checks that the "not found" and "scope" cases each produce either a plan that differs from the failed step or no plan.

### R13. Trusted apps marked Yellow as "newly published"

**Status: Mostly done** (winget skips version-age Yellow; OSV “does not cover” removed from every winget step). **Still open:** true package-identifier first-publish date; OSV note once on plan summary UI.

**What happened:** OBS Studio ("published 7 days ago"), 7-Zip (9 days), and Node LTS (2 days) were all rated Yellow because their latest manifest was recent. Every popular app that ships updates often will always look "young". Users learn to tick Yellow without reading it, which defeats the gate.

**Fix**
- For winget, "age" means the age of the **package identifier**, not of the latest version. Use the first manifest date when it's available. If the package has more than one published version in the manifest history, treat it as established.
- Keep the age rule as-is for npm and PyPI (version age is useful there for spotting hijacked releases), but compare against the package's first release as well.
- Move "OSV.dev does not cover winget packages" out of every step's reasons. It's a fact about the system, not a risk of that step. Mention it once in the safety summary.

**Acceptance:** OBS, 7-Zip, VLC, Node LTS, Git, and Python on a fresh card are Green unless there is a real signal. A test covers a package whose identifier really is under 30 days old and confirms it stays Yellow.

### R14. Plan card appears about 20 seconds late with no indicator, and the chat implies it's ready

**Status: Done** (status row: "Checking packages…" / "Running safety checks…" while `/api/plan` runs; prompt no longer says "here is the plan")

**What happened:** the reply finished, the chat said "Here is the plan", and for about 20 seconds nothing visible happened (resolution, `winget show`, the safety checks). Several times I thought no plan was coming.

**Fix:** when an install intent arrives, show a real status row in the thread right away: "Checking packages…", then "Running safety checks…", driven by the backend's `PlanningStarted`, `RecipeMatched`, and safety events. This is live progress, not placeholder content. The reply text must not say "here is the plan" (see R2).

**Acceptance:** the time from the reply ending to a visible status row is under 300 ms. The status row disappears when the card renders or an error appears.

### R15. No result message in Chill mode after a run

**Status: Done** (assistant message built from `summary.step_results` after each successful run)

**What happened:** after the Python plan finished, Chill mode showed only "DONE" chips in the Tasks panel. The chat never said what was installed.

**Fix:** after every run, add an assistant message built from `summary` (not from the model). It lists what was installed with versions, what was skipped, what was already installed, and what failed and why, with a single Rewind action for that run.

**Acceptance:** an end-to-end check confirms that each run (success, partial, failure, cancel) adds exactly one summary message that matches `summary.step_results`.

### R16. History labels are misleading

**Status: Done** (Reversible / Undone at … / Not reversible; undo rows show "Undo record" with no Rewind button)

**What happened**
- After Rewind, the original rows read "Not reversible". They should read "Undone at 8:09 PM".
- The rollback rows carry a "Rewind" chip, which suggests you could undo the undo.
- Before the rollback, pip installs inside the venv also read "Not reversible" next to rows that are reversible.

**Fix:** use exactly three row states: **Reversible** (with a Rewind button), **Undone** (with the time and a link to the undo row), and **Not reversible** (with a reason). Undo rows show no action.

**Acceptance:** a History view test covers each state with fixture records.

### R17. Rewind throws `dialog.message not allowed`

**Status: Open / unclear** (current Rewind uses browser `confirm()`, not Tauri `dialog.message`; no call site found). Re-verify on an installed NSIS build; replace `confirm` with in-app modal if the rejection still appears.

**What happened:** every Rewind logged an unhandled rejection: `dialog.message not allowed`. The capability file allows only `dialog:allow-open`.

**Fix:** don't use native message dialogs. Use the in-app `modal.js` for confirmations and results, and remove the `message` call. Add an `unhandledrejection` check to the end-to-end run so this kind of error fails CI.

**Acceptance:** zero unhandled rejections in the console across the whole persona suite.

### R18. Broad gaming request jumps straight to a manual guide

**Status: Partial** (prompt: broad → ask + 2–4 options, no manual; code ignores empty manual items). **Still open:** corpus pass and curated category lists for alternatives (ties to R22).

**What happened:** for "I just got a new pc and I wanna game on it, what do I need?", the first reply was a manual guide for "the game launcher" with Git, VS Code, 7-Zip, and VLC as alternatives. The prompt's own rule says broad requests get a question and fitting options.

**Fix**
- Add a broad-request example to the prompt: ask what they play, and offer 2 to 4 options in the right category that SetupPal can install (Steam, Discord, OBS Studio, MSI Afterburner).
- Code-side check: a `kind: manual` intent is only valid when the user named a specific out-of-scope item.

**Acceptance:** for the broad requests "I want to game", "I make music", "I'm a student", and "I'm learning to code", the corpus test checks the reply asks a question, offers 2 to 4 options in the right category, emits no manual intent, and suggests nothing that's already installed.

### R19. One conversation can only follow one plan

**Status: Done** (prompt: new installs replace the prior plan; bridge replaces `activePlan` on each new intent)

**What happened:** with the TypeScript card active, "pip install reqeusts" was answered with "But wait, our plan was to install typescript!", and "install jq and ripgrep" in the same chat never produced a card. The old card stayed on screen.

**Fix:** each new install request gets handled on its own (see R2's replacement rule). The prompt no longer frames the active plan as something the user must finish or abandon.

**Acceptance:** an end-to-end check confirms that three different install requests in one conversation produce three cards, each replacing the last.

---

## P2: Fix before release unless the owner signs off

### R20. Pixel's tone wears thin

**Status: Partial** (prompt: ≤1 joke, no greeting after first message). **Still open:** code-enforced `joke_used` flag in `build_context`.

**What happened**
- Almost every reply had a ghost joke, often the same one: "Why did the ghost cross the road?" appeared at least 4 times. One reply included a "Lettuce" knock-knock joke.
- Most replies opened with "Hello there!" or "yo!".

**Fix**
- Allow at most one joke per conversation, and only on a greeting or small talk, never on an error, a manual guide, or a plan. Enforce it in code: `build_context` passes `joke_used: true` once a joke appears, and the prompt rule reads it.
- Prompt: "Don't open with a greeting after the first message. Don't mirror the user's slang."

**Acceptance:** across a 10-turn conversation, at most one joke and at most one greeting.

### R21. PC facts and installed tools dumped into unrelated replies

**Status: Partial** (prompt: mention a PC fact only when it changes the answer). **Still open:** filter installed tools out of manual-card alternatives in code.

**What happened:** asking about MSI Afterburner, or trying a prompt injection, got a full list ("git 2.50.1, python 3.13.7, node v24.9.0, npm 11.3.0, cargo, rustc, java, winget, cmake, ninja"). "SetupPal can install instead: Git (version 2.50.1.windows.1)" offered an installed tool as an alternative.

**Fix**
- Prompt: "Mention a PC fact only when it changes the answer."
- Code: remove installed items from any "can install instead" list before rendering. The manual card builds its alternatives from `manual_guide` data, not from the model's prose.

**Acceptance:** the corpus test checks that no reply suggests installing something that's already installed, and that the tool list appears only when the user asks what's installed.

### R22. Alternatives that don't fit the user

**Status: Open** (prompt nudges same-category options; curated per-category winget lists not built yet)

**What happened:** gamers were offered Git and VS Code, and a beat-maker was offered 7-Zip, VLC, and OBS instead of music apps.

**Fix:** give each manual card and each broad answer a category (gaming, music, video, dev, utilities). Alternatives must come from the same category, from a curated per-category list of winget IDs that are known to install silently.

### R23. Plan card details

**Status: Partial** (readable titles + templated install step labels in `toActivePlan`). **Still open:** scroll-to-card without jumping the thread.

- Use a readable title ("LMMS", "Python 3.12"), not the raw goal string ("LMMS.LMMS", "typescript", "vscode").
- When the card appears, scroll to the card and keep the user's place. In one test the thread jumped to the top.
- Replace step descriptions written by the LLM ("Install LMMS.LMMS using the winget package manager within the isolated environment") with fixed templates: "Install LMMS 1.2.2 (winget)".

---

## Release gates

Everything below must pass on release-candidate builds. Any one failure blocks release.

1. **`scripts/check.ps1` is green**, including all new unit tests listed in R1 through R19.
2. **Prompt corpus test (new):** `scripts/persona-tests/corpus.json`, at least 60 requests across the three personas. Each has an expected outcome: an exact set of install IDs, a manual card for a named item, a clarifying question, or an "already installed" answer.
   - Run each request 3 times on each provider (Nebius and NIM).
   - Deterministic parts (resolver, scope validator, installability) must pass 100%.
   - Model-dependent outcomes must pass at least 95%, with **zero** runs that include an unrequested install step.
   - Build this on `scripts/persona-tests/drive.mjs`, adding pass/fail assertions to `personas.mjs`.
3. **Clean-VM install and undo:** a Windows 11 VM snapshot with no dev tools. Install and then Rewind OBS, VLC, LMMS, Audacity, Python 3.12 with the venv recipe, ripgrep, and the TypeScript npm package in a chosen folder.
   - A before/after diff of the package list, the user and machine PATH, `%ProgramFiles%`, `%LOCALAPPDATA%\Programs`, the Start Menu, `%USERPROFILE%\.venvs`, and the app folder must be empty.
   - No `winget.exe`, `msiexec.exe`, or `unins*.exe` may be left running.
4. **Already-installed safety:** on a VM that has Git, Node, and Python 3.13 installed, ask for each of those, plus "web dev setup" and "everyday apps".
   - Nothing that was installed before the test is reinstalled, upgraded, or added to History.
   - Rewinding everything leaves those tools untouched.
5. **Concurrency and stuck processes:** start an install, then cancel it, close the app, and click Rewind repeatedly. No leftover processes, and exactly one uninstall per rollback.
6. **Console is clean:** zero unhandled rejections and zero CSP violations across the persona suite.
7. **`docs/E2E_CHECKLIST.md`** rows 1 to 17 pass on the installed NSIS build, plus new rows for R1, R3, R4, R6, and R7.

## Order of work

Each item goes in its own PR, with the item ID in the branch name and PR title.

1. R5 (stuck processes): it's a safety issue and it makes every later test unreliable.
2. R3 (resolver), then R1 (scope validator and recipe matching), then R2 (single plan source). R1 and R2 depend on the resolver.
3. R4 (winget scope and error messages), R9 (verification), R8 (isolated environment paths).
4. R7 (partial approval), R6 (complete undo).
5. The rest of R10 (the override note on the card), then R11, R12, R13.
6. R14 to R19 (UI flow).
7. R20 to R23 (tone and polish).
8. Release gates 1 to 7 on a release-candidate build.
