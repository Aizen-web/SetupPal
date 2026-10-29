## Learned User Preferences

- Scope: SetupPal installs anything that (a) can be installed unattended from the command line (winget silent, npm, pip, cargo, Modrinth/Fabric through its CLI installer, and so on) and (b) passes the Safety Engine. This is not limited to dev tools. Anything else (for example apps behind proprietary GUI launchers, DRM, or interactive account logins such as Steam, Epic, or Minecraft Bedrock) gets a "Manual setup guide" reply: explain that the app does not allow non-terminal setup, give step-by-step instructions, and list similar things it can install.
- No placeholders anywhere in the app (chat, History, tests, or canned defaults)—only real data, real AI responses, and actual errors.
- Use a single fixed model per provider rather than a model picker or live model list in settings.
- The assistant should be unbiased with no predetermined output, and open broad requests by asking what the user wants installed or set up.
- Prefer adaptive install planning: ask only the clarifying questions that are needed, not a fixed info-gathering script.
- Optimize prompts for a modest (~30-40B) model: plain, best-practice, easy to understand, with occasional light flavor; do not over-babysit it.
- Assistant answers should give clear, concrete recommendations of what it can set up.
- Keep conversation/context memory compact to limit tokens while retaining what the user said.
- Account/settings should make API key paste easy and expose practical controls such as context-window viewing.
- Pixel is the only mascot (Mochi, Byte, and the pet selector are removed); Pixel's chat bubbles should be ambiguous and fun, never quoting the user's messages.
- Prefer a calm decluttered chat UI (light pet vibe): one focal idea per screen, collapse the empty Tasks panel, and keep Pixel as the only pet in view.

## Learned Workspace Facts

- The app (SetupPal) is a zero-trust orchestrator with an AI chat window, settings, and the animated Pixel mascot. The target shell is a real Tauri v2 app; `FINISH_PLAN.md` is the implementation roadmap.
- Automated CLI gate: only installs that support silent unattended flags and CLI verification (e.g. `node -v`, `python --version`, `rustc --version`, `winget list`, `pip show`) pass zero-trust gates; everything else falls back to the manual setup guide.
- AI providers: Nebius in production and NVIDIA NIM for testing, each pinned to one fixed model ID. Selection is via `SETUPPAL_PROVIDER=nebius|nim`, defaulting to Nebius when its key exists. Key handling must be done properly and keys are never written into docs or memory.
- The assistant is expected to have access to the user's PC specs when recommending installs.
- History and other UI surfaces must use real persisted app state, not demo or placeholder content.
