## Learned User Preferences

- Scope restriction: Clearly state that the orchestrator ONLY installs tools that support unattended CLI setup (web technologies, programming languages, compilers, runtimes, SDKs, package managers, dev tools). It does not install desktop apps requiring proprietary GUI launchers or interactive account logins (e.g., games with launchers like Minecraft Bedrock, Steam, Epic).
- Chat output should show only real AI responses and actual errors, never placeholder or canned default outputs.
- Use a single fixed model rather than a model picker in settings.
- The assistant should be unbiased with no predetermined output, and open broad requests by asking what the user wants installed or set up.
- Optimize prompts for a modest (~30-40B) model: plain, best-practice, easy to understand, with occasional light flavor; do not over-babysit it.
- Assistant answers should give clear, concrete recommendations of what it can set up.
- Pet companions have personalities (e.g. Pixel); their chat bubbles should be ambiguous and fun, never quoting the user's messages.
- Revamp UI work should focus on readability, looks, and choosing what information to show the user.

## Learned Workspace Facts

- The app is a zero-trust orchestrator with an AI chat window, settings, and animated pet companions.
- Strictly automated CLI environments: Only packages that support silent unattended flags and CLI verification commands (e.g. `node -v`, `python --version`, `rustc --version`) pass zero-trust gates.
- The AI backend uses an NVIDIA developer API key; key handling must be done properly and never written into docs or memory.
- The assistant is expected to have access to the user's PC specs when recommending installs.
