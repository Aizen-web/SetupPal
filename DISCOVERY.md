# SetupPal discovery

## Stack and entry points

- The desktop app uses Tauri v2 with a Svelte-free, vanilla JavaScript frontend and Vite. Rust is the backend/domain language.
- Cargo is used for Rust dependencies and npm for the frontend.
- UI: `crates/gui/src`; Rust desktop/backend: `crates/gui/src-tauri`; shared domain logic: `crates/core`.
- Common commands: `cargo test --workspace`, `cargo check --workspace`, `npm --prefix crates/gui run build`, and `npm --prefix crates/gui run check:animations`.
- The existing frontend talks to `/api` endpoints through `crates/gui/src/api/client.js`. The backend exposes those endpoints through Axum in `crates/gui/src-tauri/src/main.rs`.

## Current AI and chat flow

- The chat UI uses `/api/chat/stream`, an Axum SSE endpoint that forwards OpenAI-compatible provider deltas. The legacy JSON `/api/chat` endpoint remains available.
- NIM and Nebius provider configuration is resolved in the Rust backend; planning remains separate and uses `installer_core::NimClient`, JSON Schema enforcement, and the deterministic planner/safety/executor.
- The frontend stores conversations locally through `crates/gui/src/state/store.js`. `bridge.js` queues chat turns per conversation, streams updates, and supports cancellation; `chat-view.js` renders them.
- Existing Tavily integration is a Rust `TavilyVerifier` used by the safety engine to verify candidate source domains. It is not a general chat search tool.

## Credential handling

- `CoreConfig::from_env` reads `.env`, `NIM_API_KEY`, `NEBIUS_API_KEY`, and `TAVILY_API_KEY`.
- NIM and Tavily keys can be kept in browser local storage; chat sends the NIM key in its request. Nebius is configured from the local environment and is not stored in browser state. This is not OS-keychain storage.
- `.env.example` contains configuration names with blank secret values. Keep actual credentials in the ignored `.env`; do not commit them.
- Nebius environment configuration, provider selection, and model-list/test endpoints are implemented.

## Existing UI surfaces

- Theme/mode state: `crates/gui/src/state/store.js`; Chill/Hacker tokens: `crates/gui/src/styles/tokens.css`.
- Mascot states and animation definitions: `crates/gui/src/mascot/animations.js`, `pets.js`, and `ghost.css`; animation consistency check: `crates/gui/scripts/check-animations.js`.
- Conversation/task state: `crates/gui/src/state/store.js` and `simulation.js`; task panel: `components/task-panel.js`.
- Accounts already has API-key cards in `components/accounts-view.js`. Settings has appearance/mode controls, reduced-motion preference, and chat settings in `components/settings-view.js`.
- Settings exposes Off/Subtle/Lively ambient background intensity. CSS layers follow the existing Chill/Hacker theme and respect reduced-motion preferences.
- The chat header selects the active mascot; Pixel, Mochi, and Byte each provide a distinct backend response persona.
- The chat Stop control aborts the active response stream; task Stop remains connected to task simulation.

## Implementation outline

1. Keep provider credentials out of logs, browser-visible provider state, and committed files.
2. Verify NIM/Nebius streamed chat with controlled test keys before release.
3. Add general chat search and OS-keychain storage as separate, explicitly scoped features.
4. Preserve existing tokens and motion/reduced-motion conventions in future UI changes.

## Constraints to verify during implementation

- Streamed chat and theme-aware CSS background effects are implemented. Tavily chat tool calls, OS-keychain storage, message metrics, and the requested dynamic canvas background remain separate work.
- Nebius credentials are not present in the environment supplied to this session. Provider code can be implemented and tested with mocks, but a live connection cannot be verified without a key.
- No existing platform keychain integration is configured in the Tauri app.
