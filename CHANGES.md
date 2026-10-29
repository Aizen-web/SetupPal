# UI Polish Changes

## Completed

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
