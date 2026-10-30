## Learned User Preferences

- Scope: SetupPal installs anything that (a) can be installed unattended from the command line (winget silent, npm, pip, cargo, Modrinth/Fabric through its CLI installer, and so on) and (b) passes the Safety Engine. This is not limited to dev tools. If an app is trusted through winget (in the winget community repo with a silent installer), it is installable, including game launchers and apps that ask for a sign-in after install (Steam, Epic, Roblox, Discord, LMMS). Having a GUI is never a reason to refuse. Only things that fail the gates (Microsoft Store-only apps such as Minecraft Bedrock, vendor-page drivers, apps with no unattended installer, or anything the Safety Engine blocks) get a "Manual setup guide" reply: step-by-step instructions plus similar things it can install. The backend gate decides; the model's opinion never does.
- Release bar: SetupPal installs exactly what the user asked for, nothing extra, never recommends or touches what is already installed or unneeded, and Rewind leaves no leftovers. Chat text and the plan card must agree (the card is the plan). `RELEASE_PLAN.md` lists the blockers.
- No placeholders anywhere in the app (chat, History, tests, or canned defaults)—only real data, real AI responses, and actual errors.
- Use a single fixed model per provider rather than a model picker or live model list in settings.
- The assistant should be unbiased with no predetermined output, and open broad requests by asking what the user wants installed or set up.
- Prefer adaptive install planning: ask only the clarifying questions that are needed, not a fixed info-gathering script.
- Optimize prompts for a modest (~30-40B) model: plain, best-practice, easy to understand, with occasional light flavor; keep the system prompt ≤350 words; do not over-babysit it.
- Assistant answers should give clear, concrete recommendations of what it can set up.
- Keep conversation/context memory compact to limit tokens while retaining what the user said.
- Account/settings should make API key paste easy and expose practical controls such as context-window viewing.
- Pixel is the only mascot (Mochi, Byte, and the pet selector are removed); Pixel's chat bubbles should be ambiguous and fun, never quoting the user's messages.
- Prefer a calm decluttered chat UI (light pet vibe): one focal idea per screen, collapse the empty Tasks panel, and keep Pixel as the only pet in view.

## Learned Workspace Facts

- The app (SetupPal) is a zero-trust orchestrator with an AI chat window, settings, and the animated Pixel mascot. The target shell is a real Tauri v2 app; `FINISH_PLAN.md` is the implementation history and `RELEASE_PLAN.md` is the list of what must be fixed before release.
- Automated CLI gate: installs that run unattended and verify from the command line (e.g. `winget list`, `node -v`, `python --version`, `pip show`) pass. The gate lives in `crates/core/src/installability.rs` and judges winget packages by their manifest (installer type and source), not by a name list.
- AI providers: Nebius in production and NVIDIA NIM for testing, each pinned to one fixed model ID. Selection is via `SETUPPAL_PROVIDER=nebius|nim`, defaulting to Nebius when its key exists. Key handling must be done properly and keys are never written into docs or memory.
- The assistant is expected to have access to the user's PC specs when recommending installs.
- History and other UI surfaces must use real persisted app state, not demo or placeholder content.
- Release verification runs through `scripts/check.ps1` (workspace tests, build, animation/key/offline checks) and must stay green before calling the app release-ready.
