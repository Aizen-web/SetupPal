# SetupPal UI Polish Audit

## Status

The shared UI polish pass is implemented across the chat, task panel, Mascot
Lab, History, Downloads, Accounts, and Settings views. See [CHANGES.md](./CHANGES.md)
for the change summary and validation details.

| Area | Implemented | Remaining |
|---|---|---|
| Design and themes | Shared color, spacing, motion, and typography tokens; theme-aware speech bubbles, safety/demo surfaces, and task states. | Full contrast and interaction-state audit across every page. |
| Chat and composer | Improved message spacing/contrast and scroll behavior; composer stays available in Hacker mode; working state has a Stop action; inline UI glyphs use SVGs. | Full end-to-end send/stop behavior against a running backend. |
| Mascot Lab | Central 20-state registry; sidebar count derives from it; registry checker; lab mode, speed, loop, and replay controls. | Full choreography and per-mascot preview/coverage requirements are not complete. |
| Mascot motion | Improved `wave-hello`, reduced-motion behavior, and pause when the document is hidden. | Review every registered animation for distinct timing, pivots, and overlap on every mascot. |
| Other views | Replaced the reviewed Settings, History, Accounts, Downloads, and task-panel UI emoji/glyph icons with SVGs. | Exhaustive visual and accessibility review on every page and viewport. |
| Responsive checks | Chat checked in Chill and Hacker at 900×600, 1280×720, and 1920×1080 CSS pixels; no horizontal overflow in those checks. | Repeat full page-by-page checks in the native application. |

## Validation limitations

The frontend production build and animation-registry check pass. Browser
inspection used the Vite frontend preview; its backend health requests returned
502 because the Rust backend was not running. Therefore native launch,
backend-dependent interactions, and end-to-end behavior were not validated.
Screens were inspected in the browser, but no screenshot files were saved.
Performance profiling and exhaustive contrast/accessibility checks remain
outstanding; none are claimed as complete.
