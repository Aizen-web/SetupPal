# UI Polish & Scope Alignment Changes

## Streaming and UI refinement

- Added `/api/chat/stream` with provider SSE parsing and bounded retries for transient connection, rate-limit, and server errors before streaming starts.
- Switched the chat composer to stream response deltas, queue messages per conversation, support Stop without interrupting other conversations, and retain partial responses. Interrupted, stopped, and failed replies are excluded from later model history.
- Persisted streamed conversation updates periodically and recovered an interrupted response as stopped after restart. Chat auto-scroll now follows new text only when the user is already near the bottom.
- Added subtle theme-aware ambient backgrounds with Off, Subtle, and Lively controls in Settings, plus pointer-gated hover polish and reduced-motion handling.
- Added a chat-header mascot selector and distinct Pixel, Mochi, and Byte response personalities. New messages retain the mascot identity that answered them.
- Sending a message now explicitly anchors the chat to the latest turn rather than restoring an old scroll position.

### Validation

- `npm --prefix crates/gui run build` — passed.
- Chunk-boundary/UTF-8 SSE parser check, JavaScript syntax checks, and `git diff --check` — passed.
- Browser preview confirmed Chat and Settings render; backend-dependent calls were unavailable because the Rust backend was not running.
- `cargo check --manifest-path crates/gui/src-tauri/Cargo.toml` — blocked because `link.exe` is missing from this Windows environment.
- No live provider requests were made.

## Provider and credentials update

- Added Nebius Token Factory environment configuration, Nebius-first automatic chat and planning when `NEBIUS_API_KEY` is configured, a backend model-list endpoint, and live Nemotron model selection when `NEBIUS_MODEL` is blank.
- Added an Accounts status/test/model-list surface for Nebius without persisting its key in browser storage. Added backend environment-key status flags without returning masked or raw keys.
- Updated the Tavily verifier to use Tavily's documented Authorization header and avoid including response bodies in API errors.
- Redacted provider credentials from the Debug output of `CoreConfig` and `HttpTavilyVerifier`, with regression tests.
- Recreated `.env.example` with blank secret placeholders and documented local `.env` setup in README.
- Added `DISCOVERY.md` describing the actual app architecture and current credential/chat flow.

## Validation for this update

- `npm --prefix crates/gui run build` — passed.
- `cargo check --workspace` — blocked because the installed Windows Rust target cannot find `link.exe` (Visual C++ Build Tools are not available in PATH).
- `cargo fmt --all -- --check` — reports existing formatting differences across unrelated workspace files; no repository-wide formatting was applied.
- `npm --prefix crates/gui run check:animations` — fails because the existing sidebar animation count is not derived from `ANIMATIONS.length` (this update did not touch the sidebar or animation registry).
- A local full-history pattern scan found NVIDIA- and Tavily-shaped credential values in committed `.env.example` revisions. Rotate both keys; blanking the current example does not remove the historical copies. `gitleaks` is not installed, so this was a targeted provider-pattern scan rather than a full gitleaks report.

## Not completed or not verified

- Nebius streaming is implemented through the shared provider-compatible stream route but has not been live-tested. Tavily chat tool-call orchestration, OS keychain storage, message metrics, and a dynamic canvas background are not implemented.
- No live provider requests were made. The Nebius key was not available to this session, and no credentials were read from `.env`.
- Rust compilation could not be completed due the missing MSVC linker.

## Completed

- **CLI-Only Setup Scope Boundary**: Refocused orchestrator scope exclusively to software that supports unattended Command-Line Interface (CLI) installation and verification (web stacks, programming languages, compilers, runtimes, SDKs, package managers, and developer utilities).
- **Launcher & Store Games Exclusion**: Explicitly excluded desktop apps requiring proprietary GUI launchers, interactive store account logins, or DRM wrappers (e.g. Minecraft Bedrock, Steam, Riot, Epic). Updated AI system prompt, host facts generator, UI cards, and suggestion chips to clearly inform the user that only unattended CLI setups are automated and verified.
- **New Developer Recipes**: Added `web_fullstack_ts.yaml` (Node.js LTS, Git, NPM verification) and `rust_dev.yaml` (Rustup, Cargo, rustc verification) to deterministic recipes.
- Added shared design and motion tokens, refined chat spacing and user-message
  contrast, improved scrolling, and made speech bubbles and safety/demo surfaces
  respond to Chill and Hacker themes.
- Kept the composer available in Hacker mode and added working-state text and a
  Stop control.
- Added a shared inline SVG icon helper and replaced the reviewed emoji/text
  glyphs in the chat controls, task panel, mascot selectors, and Settings,
  History, Accounts, and Downloads views.
- Consolidated the Mascot Lab's 20 animation states in a registry. Sidebar and
  lab counts now derive from it, and
  `npm --prefix crates/gui run check:animations` checks registry IDs,
  state/keyframe selectors, and displayed counts.
- Added lab mode, speed, loop, and replay controls; improved `wave-hello`;
  added reduced-motion handling and pause-on-hidden-page behavior.
- Updated [AUDIT.md](./AUDIT.md) to track remaining work and verification limits.

## Validation

- `npm --prefix crates/gui run build` — passed.
- `npm --prefix crates/gui run check:animations` — passed (20 unique states).
- `git diff --check` — passed.
- Browser checks at 900×600, 1280×720, and 1920×1080 CSS pixels in both themes
  found no horizontal overflow in Chat; the composer and task panel remained
  visible. Settings' API-key visibility toggle worked, and History, Downloads,
  and Accounts rendered SVGs with `viewBox` attributes.

## Not completed or not verified

- The browser used a frontend-only Vite preview. Its backend health checks
  returned 502 because the Rust backend was not running, so native launch and
  backend-dependent end-to-end behavior were not verified.
- The full animation-choreography pass and all per-mascot preview/coverage
  requirements remain outstanding.
- Not every page was checked at every target viewport. No screenshots were
  saved, and performance profiling and an exhaustive contrast/accessibility
  audit were not performed.
