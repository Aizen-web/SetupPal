# SetupPal: App UI Guide (v0.3)

Applies to: the desktop app (a Tauri v2 window around the same web UI). The website reuses the same tokens, mascot and components.
Supersedes: v0.2 of this file and `SetupPal-UI-Style-Guide.md` (v0.1).

> **Pixel bubbles are flavor only.** They are short, ambiguous, and fun. They never state install facts and **never quote or paraphrase the user**. Facts belong in the plan card, Task panel, terminal, and History (see section 11.3).

---

## 0. What changed

### v0.3 (Phase 6 UI polish)

| Topic | v0.2 | v0.3 |
|---|---|---|
| Scope | Dev tools | **Anything that installs unattended from the command line** (winget, npm, pip, cargo, CLI mod installers...) and passes the Safety Engine. Everything else gets a **Manual setup guide** reply (section 6.4) |
| Mascot | Pixel only "for now" | **Pixel is the only mascot.** Byte, Mochi and the pet selector are gone, including in the Mascot Lab |
| Speech bubbles | Templates filled with real check results | **Flavor only:** short, vague, fun lines. Never facts, never the user's words (section 11.3). Facts live in the plan card, Task panel and terminal |
| Pixel placement | 72 px docked mascot during chat | Hero (140 px) on the empty chat; **40 px docked in the top bar** once a chat has messages, with a one-line bubble |
| Composer | Single-line input | **Textarea** that grows to 6 lines. Enter sends, Shift+Enter adds a line. `/` button opens the command menu |
| Slash commands | Planned list | **Built:** `/new`, `/undo`, `/help`, `/hack`, `/chill` (section 6.5) |
| Message actions | Copy, Regenerate, Edit, Thumbs | **Copy** on every reply, **Retry** on the latest reply ("Try again" on a failed one) |
| Hacker Mode | Tab strip planned | **Terminal \| Chat** tab strip is live; auto mode keeps a **2 s minimum dwell** |
| Task panel | Always visible with plan | Exists only while a plan exists, **collapses to a slim rail**; no decorative art |
| History | Two sub-tabs | One page with **search** across changes and conversations |
| Recent list | Status icons planned | **Status icon per chat** (replying, running, held, blocked, failed, finished, plan ready, chat) |
| Settings | Large wish list | Only **working** controls (section 9.5). The safety level is stored and sent with execute requests |
| Models | Model picker | **One fixed model per provider** (Nebius in production, NVIDIA NIM for testing). No picker anywhere |
| Plan approval | `Approve 4 safe steps` | Per-step approval with **"Run N approved steps"** is the target (Phase 1); today's card says "Approve safe steps" |
| Demo content | Demo mode suggested | **No demo, canned, or placeholder content anywhere.** Empty states say plainly that nothing exists yet |
| Accessibility | Guidelines | Nav landmark + `aria-current`, focus-trapped dialogs, focus kept across re-renders, text selectable everywhere except chrome |

### v0.2 (from v0.1)

| Topic | v0.1 | v0.2 |
|---|---|---|
| Product shape | Installer wizard with a mascot panel | **Chatbot** that can also call tools and run task plans |
| Light mode | Lavender-tinted "Chill" | **Warm cream / beige / caramel "Chill"** (no lavender UI accents) |
| Hacker Mode | Manual toggle | **Auto-enters when the AI starts executing tools or tasks**, manual override available |
| Pages | Home, Repos, History, Library, Settings | **Chats, History, Downloads, Accounts, Settings** |
| Task progress | Progress bar | **Task plan + Task bar** (AI defines tasks first, then ticks them off) |
| Mascot | Pixel (ghost) + Byte (cat) | **Pixel the ghost only** for now (Byte is parked) |
| Animations | List of states | **Full animation catalogue + how-to for building them as SVG** |
| Sound | Optional line | **Event map + specs; you choose the actual sounds** |

Style name stays: **Cozy Kawaii Pixel** (kawaii mascot, lo-fi cozy warmth, pixel-font headings, flat soft UI). The Chill-to-Hacker jump is the "cyber-cozy" part.

---

## 1. Principles

1. **It's a chatbot first.** Everything a normal chat assistant does must work (section 6). Tools are an extension of chat, not a separate app.
2. **Plan, then act.** For anything that changes the machine, the AI defines tasks first, shows them, and waits for approval.
3. **Chill to think, Hacker to work.** Conversation and planning are warm and calm. Execution is the neon terminal.
4. **Facts come from the code, flavor comes from Pixel.** Verdicts, hashes and step results appear only in the plan card, Task panel, terminal and History, straight from the backend. Pixel's bubbles are mood, never claims.
5. **Colour is never the only signal.** Verdicts and task states always have a word and an icon.
6. **Always a way out.** Stop (Esc), Skip task, and Rewind are always reachable.

---

## 2. Modes

### 2.1 Chill Mode (warm cream, default)
Conversation, planning, plan review, browsing History / Downloads / Accounts / Settings.

### 2.2 Hacker Mode (neon terminal)
Task execution and tool-heavy work: downloads, hash checks, installs, repo scans, test runs.

### 2.3 Automatic switching

| Event | Mode |
|---|---|
| App start, new chat, normal Q&A | Chill |
| AI is drafting the task plan | Chill (ghost `thinking`) |
| Plan shown for approval | Chill ("take a look before I touch anything") |
| User approves, first task starts `running` | **Hacker** (glasses go on) |
| A tool runs while the user only asked a question (e.g. quick web search) | Stay Chill, show a compact tool card |
| Any task is `held` (yellow) | Stay Hacker, show a banner on the Task bar |
| RED verdict | Stay Hacker, red banner, ghost `red` state |
| All tasks finished or stopped, no tool active for 3 s | **Chill** (glasses come off) |
| User sends `/hack` or `/chill` | Forced until the next task run ends |

Rules:
- **Minimum dwell:** 2 seconds in a mode before switching again (prevents flicker). Built as `MODE_MIN_DWELL_MS` in `state/store.js`; runs call `store.beginRun()` / `store.endRun()`.
- **Setting:** Settings > Appearance > Theme mode: `Auto` (default) / `Always Chill` / `Always Hacker`.
- The header Chill/Hacker toggle, `/hack` and `/chill` act like a manual override: forced until the next run ends.
- **What stays fixed during a switch:** sidebar, composer position, window chrome. Only the main area content, colours, ghost accessory and microcopy change.
- **Transition:** 300 ms cross-fade of tokens, glasses slide-on (250 ms), 120 ms scanline flicker. Sound cue optional (section 12).
- In Hacker Mode the main area has a small tab strip: **Terminal | Chat**, so the user can read the conversation without leaving the run.

---

## 3. Colour tokens

### 3.1 Chill Mode (warm cream, beige, caramel)

| Token | Hex | Use |
|---|---|---|
| `--bg-canvas` | `#F5EEDF` | App background (cream) |
| `--bg-surface` | `#FBF6EA` | Cards, message bubbles, composer |
| `--bg-sidebar` | `#EFE6D3` | Sidebar (sand) |
| `--bg-code` | `#2E2622` | Code blocks, terminal cards (warm dark) |
| `--text-code` | `#F5EEDF` | Text on code blocks |
| `--border` | `#DCCDB0` | Hairline borders |
| `--ink` | `#3B2F2A` | Primary text (cocoa) |
| `--ink-soft` | `#6F5F53` | Secondary text (contrast 5.3:1 on cream) |
| `--accent` | `#B5763F` | Fills, borders, icons, large text (caramel) |
| `--accent-strong` | `#94592B` | Text links, pressed state (contrast 4.9:1 on cream) |
| `--accent-soft` | `#F0DDC2` | Selected rows, chips, icon tiles (butter-beige) |
| `--btn-primary` | `#3B2F2A` | Primary button fill |
| `--btn-primary-text` | `#FBF6EA` | Primary button text |
| `--honey` | `#E9B949` | Decorative sparkles, highlights |
| `--peach` | `#F4C2B0` | Decorative, blush on illustrations |

Note: `--accent` (caramel) on cream is about 3.2:1, fine for fills, borders and large text, but **use `--accent-strong` for small text**.

### 3.2 Hacker Mode

| Token | Hex | Use |
|---|---|---|
| `--hx-bg` | `#06130A` | App background |
| `--hx-sidebar` | `#07170C` | Sidebar |
| `--hx-panel` | `#0B2A12` | Cards, terminal, task panel |
| `--hx-line` | `#39FF14` | Neon borders, active items |
| `--hx-line-dim` | `#1E6B12` | Panel borders at rest |
| `--hx-text` | `#5CFF3A` | Terminal text |
| `--hx-text-dim` | `#2FB81B` | Secondary terminal text |
| `--hx-glow` | `rgba(57,255,20,0.45)` | Glow on focus / active elements |

### 3.3 Verdict and task colours

| Meaning | Chill fill | Chill text | Hacker | Icon |
|---|---|---|---|---|
| GREEN / DONE | `#5E8C4A` | `#3F6B2F` | `#39FF14` | check-circle |
| YELLOW / HELD | `#D49A1A` | `#8A5A00` | `#FFD166` | alert-triangle |
| RED / BLOCKED / FAILED | `#C4493A` | `#A8372A` | `#FF6B6B` | shield-x |
| RUNNING | `#B5763F` | `#94592B` | `#5CFF3A` | loader (spinning) |
| WAITING / PENDING | `#B9A98F` | `#6F5F53` | `#2FB81B` | clock |

Badge style: fill = 18% tint of the fill colour, text = the text colour, 1 px border in the fill colour at 40%.

### 3.4 Ghost palette (Pixel)

| Part | Hex |
|---|---|
| Body | `#E6E0FF` |
| Body outline | `#BBAEFF` |
| Cheeks | `#F8BBD9` |
| Mouth | `#7B61D6` |
| Eyes | `#1E1B2E` + white highlight |
| Glasses frame (Hacker) | `#39FF14` |
| Glasses lens | `#03110A` |

The ghost is the **only cool-toned object** in warm Chill Mode. That contrast is deliberate: it makes the mascot pop.

---

## 4. Typography

| Role | Font | Fallback |
|---|---|---|
| Display / headings / logo | Pixelify Sans (or Silkscreen) | monospace |
| UI + chat text | Inter | system-ui |
| Code / terminal / Hacker speech bubble | JetBrains Mono | Consolas, monospace |
| Handwritten notes (rare) | Caveat | cursive |

Confirm font licences before bundling.

| Token | Size / line height | Use |
|---|---|---|
| `--fs-h1` | 28 / 1.2 | Page titles (pixel font) |
| `--fs-h2` | 20 / 1.3 | Card titles (pixel font) |
| `--fs-body` | 14 / 1.65 | Chat messages, UI text |
| `--fs-small` | 12 / 1.5 | Captions, metadata |
| `--fs-mono` | 13 / 1.6 | Logs, code |
| `--fs-label` | 11 / 1.2, uppercase, +0.12em | Section labels ("RECENT"), chips |

User setting: text size S / M / L scales all tokens by 0.92 / 1 / 1.12.

---

## 5. Layout and shape

- Default window 1100 x 720, minimum 900 x 600.
- Base spacing unit 4 px (4, 8, 12, 16, 24, 32).
- Radius: buttons 12, cards 16, speech bubbles 18, composer 14, chips and toggles 999.
- Borders: 1 px `--border` on cards; 2 px on speech bubbles and focused elements.
- Shadows: nearly none. Floating layers only: `0 8px 24px -12px rgba(59,47,42,0.22)`. Hacker Mode uses glow instead.
- Chat column max width 760 px, centred in the main area.

### 5.1 App shell (Chill)

```
+------------------+------------------------------------------------+-----------------+
| Setup Pal        |  [ghost]  (bubble: "Here's my plan. Take a     |  TASKS          |
| [ + New chat ]   |           look before I touch anything.")       |  2 of 4 done    |
|                  |                                                |  [=====     ]   |
| Chats            |  [ chat messages / plan card / tool cards ]    |  v Java 21      |
| History          |                                                |  ~ Fabric       |
| Downloads        |                                                |  o Sodium       |
| Accounts         |                                                |  ! FastLight    |
|                  |                                                |                 |
| RECENT           |  +------------------------------------------+  |                 |
|  v Minecraft...  |  | Tell me what to set up...        [Send]  |  |                 |
|  v C++ toolchain |  +------------------------------------------+  |                 |
| Safety: strict   |                                                |                 |
| Settings         |                                                |                 |
+------------------+------------------------------------------------+-----------------+
  240 px                          flexible                              300 px
```

### 5.2 App shell (Hacker)

```
+------------------+------------------------------------------------+-----------------+
| Setup Pal        | [ Terminal | Chat ]                            |  PROGRESS       |
| [ + New chat ]   | [ghost+glasses] "Hashes check out so far.      |  2 of 4 done    |
| ...same nav...   |   Fabric is next. Hold tight."                 |  v Java 21 DONE |
|                  | > plan approved: 4 steps, 1 held for you       |  v Launcher DONE|
| RECENT           | v java 21 . adoptium.net . sha256 matches      |  ~ Fabric RUN   |
|                  | ~ fabric loader . downloading                  |  4 Sodium WAIT  |
| Safety: strict   | ? FastLight Pro is held (yellow).              |  5 FastLight HELD|
| Settings         | +------------------------------------------+   |                 |
|                  | | Working... press Esc to stop      [Stop] |   |                 |
|                  | +------------------------------------------+   |                 |
+------------------+------------------------------------------------+-----------------+
```

The sidebar and composer stay where they are; the middle swaps from chat to terminal, and the Task bar switches to neon styling.

---

## 6. The chat experience (chatbot baseline)

Everything a normal chatbot does must work. Priority P0 = needed for the demo, P1 = soon after.

| Feature | Spec | Priority |
|---|---|---|
| Streaming replies | Token-by-token, with a stop button | P0 |
| Markdown | Headings, lists, tables, bold/italic, links, blockquotes | P0 |
| Code blocks | Language label, copy button, horizontal scroll or wrap toggle | P0 |
| Message actions | Copy (every reply, on hover/focus; pinned on the latest), Retry (latest reply; "Try again" when it failed). Edit-and-resend later | P0 |
| Stop / Esc | Stops generation and any running task safely | P0 |
| Multi-turn context | Full conversation context sent to the model, trimmed sensibly | P0 |
| Auto titles | Chat gets a short title after the first exchange; user can rename | P0 |
| New chat, switch chats | Sidebar RECENT list | P0 |
| Slash commands | Built: `/new`, `/undo`, `/help`, `/hack`, `/chill` (section 6.5) | P0 |
| Suggestion chips | Under empty composer: "get me C++", "set up this repo", "fix my PATH" | P0 |
| Source chips | When Tavily search is used, show domain chips under the answer, each with a trust icon | P0 |
| Attachments | Text and code files; images only if the selected model supports vision | P1 |
| Search chats | In History page and with `Ctrl+F` in a chat | P1 |
| Export chat | Markdown export | P1 |
| Edit-branch | Editing an old message creates a branch | P2 |
| Voice input | Mic button in composer | P2 |

### 6.1 Message styling

| Element | Chill | Hacker (Chat tab) |
|---|---|---|
| User message | `--accent-soft` bubble, right-aligned, radius 16 | dark-green bubble, neon border, right-aligned |
| Assistant message | No bubble, text on canvas, small ghost avatar (28 px) at left | Same, JetBrains Mono |
| Timestamps | On hover, `--fs-small` | Same |
| Scroll-to-bottom | Floating pill when scrolled up | Same |

### 6.2 Composer

- A `<textarea>`: one line high, grows to 6 lines, then scrolls. Radius 16, 1 px border, `Send` primary button at right.
- **Enter** sends, **Shift+Enter** adds a line (IME composition is respected).
- Left of the input: a `/` button that opens the command menu.
- States: **idle** ("What would you like to set up?"), **generating** (Stop appears; you may type another question, it queues), **running tasks** (placeholder "Working... press Esc to stop", Stop button).
- **Esc** stops a streaming reply first, then a running plan. Inside a dialog or the command menu, Esc only closes that layer.
- A quiet notice line under the composer (`aria-live`) reports command results, e.g. "Nothing to undo yet."

### 6.3 Empty state

Hero Pixel (140 px), one flavor bubble, the headline "What should we set up?", 3 starter pills, and one scope line: "SetupPal installs things that set up unattended from the command line. Anything else gets a step-by-step manual guide."

### 6.4 Manual setup guide

When something cannot be installed unattended (Microsoft Store-only apps such as Minecraft Bedrock, vendor-page drivers, or installers with no silent mode), the assistant replies with a **Manual setup guide**: one line saying why it needs a manual install, numbered steps the user can follow, and a short list of similar things SetupPal *can* install. It is a normal assistant message (no plan card, no Approve button).

### 6.5 Slash commands

| Command | Does |
|---|---|
| `/new` | Start a new chat |
| `/undo` | Roll back the latest reversible change (same as History > Rewind last change) after a confirmation dialog; reports the result under the composer |
| `/help` | Show the command menu |
| `/hack`, `/chill` | Force a mode until the next run ends |

Typing `/` opens a filtered menu (Arrow keys move, Enter runs, Tab completes, Esc closes). Commands run locally and never reach the model. Text that only *starts* with a slash, such as a path, is sent as normal. New commands are added with `registerSlashCommand()` in `components/slash-commands.js`.

---

## 7. Tool calls in the chat

The assistant can call tools mid-conversation. Every call appears as a **tool card** in the chat.

### 7.1 Tool list (v1)

| Tool id | What it does | Needs approval (Strict) |
|---|---|---|
| `web.search` | Tavily search | No |
| `web.extract` | Tavily page extract | No |
| `github.scan` | Read repo files, manifests, CI, scripts; compute trust verdict | No |
| `download.fetch` | Download from an allowlisted domain into Downloads | Yes |
| `verify.hash` | Compare SHA256 with the publisher's | No |
| `verify.signature` | Check a signature where published | No |
| `install.run` | Run an install step from an approved plan | Yes |
| `env.path_edit` | Change PATH or environment variables | Yes |
| `test.run` | Compile / run a hello-world or the repo's tests | Yes |
| `rewind.undo` | Revert logged changes | Yes |

The LLM only produces a **structured plan** (tasks + tool ids + arguments). A deterministic executor runs them and enforces the domain allowlist. The LLM never runs raw shell commands, and text from web pages or READMEs is data, never instructions.

### 7.2 Tool card anatomy

```
+--------------------------------------------------------------+
| [icon] web.search   "gcc install windows"      [DONE] 1.4 s  |
|  3 sources: gcc.gnu.org . msys2.org . learn.microsoft.com    |
|  v Details                                                   |
+--------------------------------------------------------------+
```

- **Collapsed (default):** icon, tool name, one-line summary, status chip, duration, verdict badge if any.
- **Expanded:** arguments (pretty JSON), output excerpt, full log, copy buttons.
- **Chill style:** `--bg-surface` card, 1 px border, radius 14.
- **Hacker style:** terminal lines, e.g.
  ```
  > web.search "gcc install windows"
  v 3 sources, 1.4s
  ```
- **Status chips:** RUNNING (spinner), DONE, FAILED, SKIPPED, AWAITING APPROVAL.
- Tools that need approval show an inline **Approve / Deny** button pair inside the card until answered.

### 7.3 Safety levels (the "Safety: strict" chip)

| Level | Behaviour |
|---|---|
| **Strict** (default) | Every plan needs approval; read-only tools auto-run; YELLOW tasks always held |
| **Balanced** | Read-only tools and GREEN, hash-verified downloads auto-run after plan approval; YELLOW held |
| **Relaxed** | Plan approval once per chat; YELLOW still held; RED is **always** blocked |

RED verdicts and non-allowlisted domains are blocked at every level.

The level is chosen in Settings > Safety level and shown in the sidebar chip (clicking it opens that section). It is stored in `setuppal_ui_settings` and sent with execute requests through `executePrefsPayload()` in `state/prefs.js`; the backend decides what each level allows.

---

## 8. Task system and the Task bar

### 8.1 Flow

1. User asks for something that changes their machine ("Minecraft with performance mods").
2. Ghost `thinking`; the AI **defines the tasks first**. They stream into a **Task plan card**, one by one.
3. Each task shows: number, title, source domain, check result, verdict badge (as in your mockup).
4. User can **edit**: remove a task, reorder, add a task ("also install Git"). The AI re-validates after edits.
5. Buttons: `Approve 4 safe steps` (primary) and `Review the yellow one` (secondary).
6. On approval the app auto-enters Hacker Mode and tasks execute in order.
7. The AI **always appends a final task: "Verify it works"** (compile hello-world, run `--version`, new-shell PATH check).
8. When done: celebrate animation, summary message, auto-return to Chill.

### 8.2 Task states

| State | Chip | Meaning |
|---|---|---|
| `pending` | PENDING | Defined, plan not yet approved |
| `waiting` | WAITING | Approved, queued |
| `running` | RUNNING | In progress |
| `done` | DONE | Finished and verified |
| `held` | HELD | YELLOW, waiting for the user |
| `blocked` | BLOCKED | RED, will not run |
| `failed` | FAILED | Error; AI may retry or ask |
| `skipped` | SKIPPED | User skipped it |

### 8.3 Counter rules (important for consistency)

- **Headline:** `X of N done`, where **N = approved tasks** (`waiting`, `running`, `done`, `failed`). `held`, `blocked`, `skipped` are not counted in N.
- **Sub-line:** `1 held . 0 blocked` shown only when non-zero.
- Progress bar fill = `done / N`.
- If the user approves a held task, N increases by one and the headline updates with a small bump animation.
- When N = 0 (e.g. only chatting), the Task bar is hidden.

### 8.4 Task bar placement

| Where | Chill | Hacker |
|---|---|---|
| **Right panel** ("TASKS" in Chill, "PROGRESS" in Hacker) | Exists only while a plan exists; collapses to a 52 px rail with `done/N` | Same; the user's collapse choice is kept |
| **Top mini-strip** in the main area | `2 of 4 tasks done` + thin bar; click opens the panel | Hidden (panel already visible) |
| **Inline plan card** in the chat | Interactive checklist with Approve / Review buttons | Read-only history in Chat tab |

Panel item layout: status icon (28 px circle) + task title + status chip. Running item has a spinner and a glowing border in Hacker Mode. Clicking an item scrolls the terminal to that task's log lines.

### 8.5 Task data model (for the dev)

```json
{
  "plan_id": "pl_001",
  "title": "Minecraft with performance mods",
  "safety": "strict",
  "tasks": [
    {
      "id": 1,
      "title": "Install Java 21",
      "tool": "install.run",
      "source": "adoptium.net",
      "check": "hash matches publisher",
      "verdict": "GREEN",
      "status": "done",
      "started_at": "2026-10-26T13:20:11Z",
      "ended_at": "2026-10-26T13:20:48Z",
      "reversible": true
    }
  ]
}
```

---

## 9. Pages and tabs

Sidebar order: **+ New chat**, then **Chats**, **History**, **Downloads**, **Accounts**, then RECENT list, then the Safety chip and **Settings** pinned at the bottom. Active item uses `--accent-soft` (Chill) or a neon left bar (Hacker).

### 9.1 Chats (home)
The chat described in sections 6 to 8. RECENT lists saved chats, each with a status icon derived from what the chat really holds (plus a screen-reader label): replying, running, held (yellow), blocked (red), last reply failed, finished, plan ready, or plain chat.

### 9.2 History

One page with a **search box** in the header that filters both sections (change summaries, goals, changed paths; chat titles and message text).

**Changes (Rewind log)** first, then **Past conversations** (title, message count, tasks done, last activity; Open and Delete).

Later: rename, pin, export (.md), retention setting.

**Changes (Rewind log)**
- Timeline of every change made to the machine: package installed, file written, PATH edited.
- Each entry: what, when, which task, reversibility tag: `Full`, `Partial`, `Manual`.
- Select entries and press **Rewind selected** (Danger button, confirmation dialog).
- Ghost `rewind` animation while reverting, `green` when done.

### 9.3 Downloads

- Top bar: storage used, download folder path, `Clean cache` button.
- Tabs: **All**, **Verified**, **Quarantine** (YELLOW and RED files; not runnable until reviewed), **Queue** (active downloads).
- Table columns:

| Column | Content |
|---|---|
| File | Name + icon |
| Source | Domain + "allowlisted" badge |
| Size | Human readable |
| SHA256 | Truncated, copy button, status (`matches publisher` / `no publisher hash` / `mismatch`) |
| Verdict | GREEN / YELLOW / RED badge |
| Date | Relative |
| Actions | Open folder, Re-verify, Install again, Delete |

- Active rows show a progress bar, speed and ETA; the ghost `downloading` animation plays in the header.
- A file with a hash mismatch is auto-moved to Quarantine and flagged RED.

### 9.4 Accounts

Sections, top to bottom:

1. **Profile:** display name, avatar (ghost colour variants), local-only vs signed-in badge. v1 can be a **local profile**; cloud sign-in is a stretch goal.
2. **Connected services** (card per service with `Connect / Disconnect` and `Test connection`):
   - **GitHub:** OAuth, read-only scopes by default; shows connected username and granted scopes.
   - **Nebius Token Factory:** API key field (masked, easy paste). One fixed model, shown read-only; no model picker.
   - **Tavily:** API key field (masked).
3. **Usage:** local counters (messages, tool calls, tokens used this month). Show only numbers the app really measured.
4. **Privacy and data:** what stays on this device, `Export my data`, `Delete all data`.
5. **Security:** keys stored in the OS keychain (never in plain config files, never in chat logs, never in the repo), sign-out everywhere (if cloud accounts exist).

Danger zone card at the bottom (red outline): delete data, reset app.

### 9.5 Settings

Only controls that work today. Everything is saved locally and applied immediately.

| Section | Contents |
|---|---|
| Appearance | Theme mode (Auto / Always Chill / Always Hacker), text size (S / M / L), reduce motion (also turns off confetti), ambient background |
| Pixel | Speech bubbles on/off, Mascot Lab (preview every animation) |
| Safety level | Strict (default) / Balanced / Relaxed, with a one-line explanation of the selected level |
| Chat & memory | Recent messages kept in full, reply length, share PC facts, the fixed model in use (read from `/api/health`), Clear all chats, Reset |
| System scan | Run the doctor scan: OS, GPU, detected dev tools |
| Sound | Sound effects on/off, volume |

Later: allowlist viewer, shortcuts list, data/retention, about.

---

## 10. Components

| Component | Spec |
|---|---|
| **Primary button** | `--btn-primary`, 40 px high, radius 12, 14 px medium, optional trailing arrow. Hover: lift 1 px. Focus: 2 px `--accent` ring |
| **Secondary button** | Transparent, 1 px `--border`, `--ink` text (Hacker: neon outline) |
| **Danger button** | Brick red fill, used for Rewind, Delete |
| **Chips** | 24 px high, pill, `--accent-soft` |
| **Status chip** | Uppercase 11 px, see 3.3 |
| **Cards** | `--bg-surface`, 1 px border, radius 16, padding 20 |
| **Icon tile** | 44 px rounded square, `--accent-soft`, 20 px Lucide icon (1.5 px stroke) |
| **Toggle** | 40 x 22 pill, on = `--accent` (Hacker: neon) |
| **Dialog** | Centered card, radius 20, 24 px padding, overlay `rgba(59,47,42,0.35)` |
| **Toast** | Bottom centre, ghost head icon, 4 s, optional Undo |
| **Tooltip** | Speech-bubble style with tail, 150 ms delay |
| **Terminal card** | `--bg-code`, three window dots, mono text, copy button, `>` prefixed lines |
| **Progress bar** | 6 px, `--accent` fill (Hacker: neon with glow) |
| **Banner** | Full-width strip under the top bar for HELD / RED events |

---

## 11. Mascot: Pixel the ghost

### 11.1 Personality
Shy, gentle, thorough. Spooky puns (max one per message), short sentences.

### 11.2 Placement

| Context | Size | Where |
|---|---|---|
| Empty chat | 140 px "hero" | Above the headline, bubble to the right |
| Chat has messages, or a run | 40 px "docked" | Left of the top bar, with a one-line pill bubble |
| Next to assistant messages | 20 px ghost icon | Avatar, author name "Pixel" |

Only one live Pixel is on screen at a time (the Mascot Lab preview is separate). Clicking Pixel pokes it.

### 11.3 Speech bubble
- Hero: max 2 lines, radius 18, 2 px border. Docked: one line, ellipsis.
- Chill: Inter on `--bg-surface`. Hacker: neon border and glow.
- **Flavor rule (overrides v0.2):** lines come from one bank in `mascot/pixel-lines.js`, keyed by event. They are short, vague and fun ("Ooh, what are we building?"), make **no factual claims** (no package names, verdicts, counts) and **never quote or paraphrase the user**. Events carry no payload, so this is enforced by code.
- Settings > Pixel > Speech bubbles turns them off entirely.

### 11.4 SVG structure (what the AI must keep stable)

One SVG, `viewBox="0 0 200 200"`, every part a named group so animations can target it:

```
#ghost-root
  #shadow            ground shadow (scales with float)
  #ghost-body
    #body            main shape
    #hem             wavy bottom edge
    #face
      #eye-l  #eye-r
      #brow-l #brow-r   (hidden by default)
      #mouth
      #cheek-l #cheek-r
  #acc-glasses       Hacker Mode sunglasses
  #acc-hardhat       working
  #acc-box           downloading
  #acc-sign          "HELD" sign
  #acc-flashlight    scanning
  #fx-sparkles  #fx-sweat  #fx-zzz  #fx-confetti  #fx-shield  #fx-dots  #fx-bandage  #fx-clock
```

All `#acc-*` and `#fx-*` groups start with `opacity: 0`.

---

## 12. Mascot animation catalogue

Priority: **P0** = build first (needed for the demo), P1 = next, P2 = polish.
Timings are starting points; tune by eye in the preview page.

| # | ID | Trigger | What it looks like | Duration / loop | Priority |
|---|---|---|---|---|---|
| 1 | `idle-float` | Default, always on | Body floats up/down 6 px, shadow shrinks as it rises, hem sways | 2.4 s, infinite | P0 |
| 2 | `blink` | Overlay on all states | Eyes scaleY to 0.1 and back | 120 ms, every 4 to 6 s (random) | P0 |
| 3 | `speaking` | Bubble is typing | Mouth squashes open/closed in time with text | 180 ms loop while typing | P0 |
| 4 | `thinking` | AI drafting a plan | Eyes look up-right, three dots (`#fx-dots`) pulse above head, body tilts 4 degrees | 1.6 s, infinite | P0 |
| 5 | `scanning` | Verifying a source or repo | Flashlight (`#acc-flashlight`) sweeps left-right, body opacity drops to 0.75, eyes follow beam | 1.8 s, infinite | P0 |
| 6 | `working` | Task running | Hard hat appears, faster float (1.2 s), tiny sparkles pop beside body | infinite | P0 |
| 7 | `glasses-on` | Enter Hacker Mode | Glasses slide down from above eyes with overshoot, lens glint sweeps once | 250 ms, once | P0 |
| 8 | `glasses-off` | Leave Hacker Mode | Reverse of above | 250 ms, once | P0 |
| 9 | `green` | GREEN verdict / task done | Eyes become happy arcs, sparkles burst, small hop | 900 ms, once | P0 |
| 10 | `yellow` | HELD / needs review | Head tilts 8 degrees, sweat drop slides down, brows tilt | 1.2 s, once, then settles | P0 |
| 11 | `red` | RED verdict | Quick shake (4 x 4 px), shield pops in front, mouth wobbles ("boo!") | 600 ms once, shield stays until dismissed | P0 |
| 12 | `celebrate` | All tasks done | Spin once, confetti burst, glasses (if on) tilt up, then back to Chill | 1.4 s, once | P0 |
| 13 | `wave-hello` | New chat, onboarding | One arm-less wave: body tilts left/right twice, sparkle | 1.2 s, once | P1 |
| 14 | `listening` | User typing | Leans toward composer, eyes widen | While typing | P1 |
| 15 | `downloading` | Download in progress | Carries a box (`#acc-box`), hem ripples faster, speed lines | infinite | P1 |
| 16 | `error` | Non-security failure | Droops 6 px, bandage appears, eyes become "x" briefly, then sad-eyes | 1 s once, then held pose | P1 |
| 17 | `held-sign` | Task is `held` | Holds up the "HELD" sign; gentle bob | infinite while held | P1 |
| 18 | `rewind` | Rewind running | Spins counter-clockwise, afterimages trail, clock (`#fx-clock`) hands run backwards | 1.4 s, repeats | P1 |
| 19 | `sleepy` | 2 min idle | Eyes half-closed, "zzz" floats up, slower float | infinite | P2 |
| 20 | `peek-a-boo` | Empty states | Ghost slides in from behind the panel edge, peeks, then fully appears | 900 ms, once | P2 |

### 12.1 State priority (when two want to play)

`red` > `error` > `yellow` > `green` > `celebrate` > `scanning` > `working` > `downloading` > `thinking` > `speaking` > `listening` > `idle`

Rules: a one-shot (`green`, `yellow`, `red`, `error`) plays fully, then returns to the underlying state. `blink` and `idle-float` run underneath everything. `glasses-*` is an accessory layer, independent of state.

---

## 13. How to build the SVG animations

### 13.1 Pick the tech (this depends on your app framework)

| App stack | Recommended approach |
|---|---|
| **Electron / Tauri / web view** | Inline the SVG in the DOM; animate with CSS `@keyframes` and a `data-state` attribute on `#ghost-root`. This is the easiest option. |
| **PyQt (QtSvg)** | QtSvg renders only a limited subset of SVG, so **CSS keyframes will not run** there. Either (a) host the mascot in a `QWebEngineView` and use the web approach, or (b) split the SVG into layer files and animate them with `QPropertyAnimation` / `QGraphicsSvgItem`. Test one animation first to confirm. |

If you're undecided on the framework, the mascot animations are the strongest argument for a web-based UI (Electron or Tauri).

### 13.2 The CSS rules that make SVG animation work

```css
#ghost-root * { transform-box: fill-box; transform-origin: center; }

@keyframes float { 0%,100% { transform: translateY(0) } 50% { transform: translateY(-6px) } }
#ghost-body { animation: float 2.4s ease-in-out infinite; }

@keyframes blink { 0%,94%,100% { transform: scaleY(1) } 97% { transform: scaleY(0.1) } }
#eye-l, #eye-r { animation: blink 5s infinite; }

/* state layer */
#ghost-root[data-state="green"] #fx-sparkles { animation: sparkle-burst .9s ease-out both; }

@media (prefers-reduced-motion: reduce) {
  #ghost-root * { animation: none !important; }
}
```

- `transform-box: fill-box` + `transform-origin: center` is required, otherwise rotations pivot around the SVG's top-left corner.
- Animate only `transform` and `opacity` (cheap and smooth).
- Use `animation-fill-mode: both` for one-shots so they hold their end pose.
- Keep timings in CSS variables (`--t-float: 2.4s`) so you can retune in one place.

### 13.3 State machine (sketch)

```js
const PRIORITY = { red:100, error:90, yellow:80, green:70, celebrate:65, scanning:50,
                   working:45, downloading:44, thinking:40, speaking:35, listening:30, idle:0 };
const ONE_SHOT = new Set(["green","yellow","red","error","celebrate","glasses-on","glasses-off"]);

function setMascot(next) {
  if (PRIORITY[next] < PRIORITY[current] && !ONE_SHOT.has(current)) return;
  root.dataset.state = next;
  if (ONE_SHOT.has(next)) root.addEventListener("animationend", () => setMascot(baseState), { once: true });
  current = next;
}
// Glasses are separate: root.dataset.mode = "hacker" | "chill"
```

### 13.4 Letting the AI make the animations

The SVG work can be handled by the AI. Workflow:

1. **Finalize one static ghost SVG** with the structure in 11.4 (the AI can create this too: ask for "the ghost with these group IDs").
2. **Ask for one animation at a time**, pasting the current SVG. Use this prompt template:

   > Here is my ghost SVG (below). Do not change existing shapes or IDs. Write CSS keyframes for the animation `{ID}` described as: `{description from the catalogue}`. Use only `transform` and `opacity`. Target only these IDs: `{list}`. Any new shapes go inside the existing empty `<g id="fx-...">`. Duration `{x}`, `{loop / once}`. Include a `prefers-reduced-motion` fallback. Output the CSS and the minimal HTML snippet to test it.

3. **Build a preview page** (one HTML file with a button per state and a mode switch). The AI can generate this; it makes review fast.
4. **Review by eye and iterate in plain words**: "less bouncy", "shake is too long", "sparkles should be warmer". The AI revises the CSS.
5. **Save each animation as its own file** (`ghost.base.css`, `ghost.green.css`, ...) so changes are small and easy to review in git.

### 13.5 QA checklist per animation
- Loops are seamless (no jump at the loop point).
- Pivot points correct (nothing rotates around a corner).
- Looks right at 28, 72, 160 and 256 px.
- Works in both modes (glasses on and off).
- Reduced-motion version shows a clear static pose.
- At most one looping state plus the idle float at a time.
- SVG stays under about 40 KB; use one glow filter (on the glasses), not many.
- Animations pause when the window is hidden (`visibilitychange`).

---

## 14. Sound design (you choose the sounds)

The app ships with the **sound engine and event map**, and **you pick the actual audio files**. Nothing in the UI depends on sound; every sound has a visual twin.

### 14.1 Event map

| Event id | When | Feel to aim for | Max length |
|---|---|---|---|
| `ui.click` | Buttons | Tiny soft tick | 80 ms |
| `msg.send` | User sends | Light "pop" | 150 ms |
| `msg.receive` | Reply finished | Soft two-note blip | 250 ms |
| `msg.typing` (optional) | Per few characters of the bubble | Very quiet pixel blip | 40 ms |
| `mode.hacker` | Enter Hacker Mode | Click + low synth sweep up (glasses on) | 500 ms |
| `mode.chill` | Leave Hacker Mode | Soft sweep down | 400 ms |
| `plan.ready` | Task plan shown | Gentle "ta-da" note | 400 ms |
| `task.start` | A task begins | Short rising blip | 150 ms |
| `task.done` | Task done | Bright "ding" | 300 ms |
| `task.all_done` | All tasks done | Short happy jingle | 1.2 s |
| `verdict.green` | GREEN | Warm chime | 400 ms |
| `verdict.yellow` | YELLOW / HELD | Curious two-note "hmm" | 450 ms |
| `verdict.red` | RED | Low buzz plus soft "boo" | 600 ms |
| `error` | Failure | Dull thud | 400 ms |
| `download.done` | Download finished | Soft "plop" | 300 ms |
| `rewind` | Rewind | Reverse whoosh | 800 ms |
| `toast` | Notification | Light bell | 300 ms |
| `ghost.sleepy` (optional) | Idle 2 min | Tiny snore | 800 ms |

### 14.2 Technical specs
- Format: `.ogg` (preferred) or `.mp3`; mono is fine for effects; 44.1 kHz.
- Trim silence at both ends; normalise so peaks sit around -3 dB; keep effects well below music-like loudness.
- Keep every file small (under about 50 KB, jingle up to about 150 KB).
- Default volume 40%; sounds off until the user finishes the first-run prompt ("Want little sounds?").
- Settings: master volume, mute, per-category toggles (UI, tasks, verdicts, mascot), "mute when window unfocused".
- Engine rules: preload all files; don't play the same event twice within 150 ms; duck other sounds under `verdict.red`.
- On web views, audio only starts after a user gesture (browser rule), so initialise the audio engine on the first click or keypress.

### 14.3 Sound pack manifest (so you can swap sounds without touching code)

```json
{
  "pack": "my-pack",
  "author": "you",
  "events": {
    "ui.click": "sounds/click.ogg",
    "task.done": "sounds/task-done.ogg",
    "verdict.red": "sounds/red.ogg"
  }
}
```

Missing events simply stay silent. Users can load a different pack from Settings > Sound.

### 14.4 Where to get sounds
- **Make your own, 8-bit style** (best fit for the pixel look, no licence worries): jsfxr, sfxr, BFXR or ChipTone generate blips and chimes in seconds.
- **Free libraries:** Freesound (filter by CC0), Kenney.nl audio packs (CC0), OpenGameArt (check each item's licence).
- Sites like Pixabay or Mixkit have their own licence terms, and some restrict redistributing the raw files, which matters when your **repo is public**. Read the terms first.
- Do not use sounds ripped from games, films or other apps.
- Record every sound's source and licence in `CREDITS.md`. CC0 or self-made keeps the open-source repo clean.

---

## 15. Motion (general)

| Element | Animation | Duration | Easing |
|---|---|---|---|
| Speech bubble in | scale 0.92 to 1 + fade | 180 ms | ease-out |
| Mode switch | token cross-fade + scanline flicker | 300 ms + 120 ms | ease |
| Task plan card | tasks stream in, 80 ms stagger | 200 ms each | ease-out |
| Task state change | chip cross-fade, icon pop | 220 ms | ease-out |
| Counter change | number rolls, bar eases | 350 ms | ease-in-out |
| Card / button hover | lift 1 to 2 px | 150 ms | ease |
| Page change | fade + 8 px slide | 180 ms | ease-out |
| Toast | slide up + fade | 200 ms | ease-out |

Reduce-motion setting: no loops, no confetti, no slides; 100 ms fades only.

---

## 16. Microcopy (Pixel's voice)

| Moment | Chill | Hacker (terminal style) |
|---|---|---|
| Greeting | "Boo... I mean, hi! What are we setting up today?" | n/a |
| Plan ready | "Here's my plan. Take a look before I touch anything." | n/a |
| Approved | "Okay, putting on my shades." | `> plan approved: 4 steps, 1 held for you` |
| Scanning | "Let me peek at this... no spooky stuff so far." | `> scanning adoptium.net` |
| GREEN | "Checking the hash... it matches! No ghosts in this installer. Boo-yah." | `> sha256 matches publisher` / `> verdict: GREEN, installing` |
| YELLOW | "Hmm, this one's a little odd. Want to look at it with me?" | `? FastLight Pro is held (yellow). Ask me later.` |
| RED | "Nope. That's a scary one. I blocked it." | `> typosquat match: requests 94%` / `> verdict: RED, blocked` |
| Progress | "Hashes check out so far. Fabric is next. Hold tight." | `~ fabric loader . downloading` |
| Done | "All set! I ran a test and it works." | `v verify: g++ hello.cpp OK` |
| Failure | "Oops, that step broke. Let me look at the error." | `x task 3 failed: exit code 1` |
| Rewind | "Rewinding... everything's back how it was." | `< reverting 3 changes` |
| Idle | "Zzz... wake me when you need a setup." | n/a |

Rules: the Hacker column is real terminal output from backend events. The Chill column is the *tone* to aim for; in-app bubbles follow the flavor rule in 11.3, so lines that name specific packages or results belong in assistant messages or the plan card, not in Pixel's bubble. Messages say what was checked ("hash matches publisher"), never just "safe". Errors say what happened and what to try next.

---

## 17. Accessibility

- Text contrast at least 4.5:1; small caramel text uses `--accent-strong`.
- Every verdict and task state has an icon and a word.
- Full keyboard use: `Tab` order follows visual order, visible 2 px focus ring, `Esc` stops a reply or run.
- Landmarks: sidebar `<nav aria-label="Main">` with `aria-current="page"`, `<main>` for the active view, the Task panel is an `<aside>`.
- Dialogs (Context window, Mascot Lab) use `role="dialog"` + `aria-modal`, trap focus, close on Esc, and return focus to the button that opened them.
- Views re-render on state changes; the focused control and caret position are restored.
- Text is selectable everywhere except chrome (buttons, nav, badges, the mascot).
- Honour OS reduce-motion and high-contrast settings; Settings > Reduce motion applies the same rules in-app.
- Mascot images have alt text describing state ("Pixel the ghost, happy, task finished").
- Sounds are never the only signal.
- Hacker glow is decoration only.

---

## 18. Implementation notes

### 18.1 Tokens (CSS variables)

```css
:root {
  --bg-canvas:#F5EEDF; --bg-surface:#FBF6EA; --bg-sidebar:#EFE6D3;
  --bg-code:#2E2622; --text-code:#F5EEDF;
  --border:#DCCDB0; --ink:#3B2F2A; --ink-soft:#6F5F53;
  --accent:#B5763F; --accent-strong:#94592B; --accent-soft:#F0DDC2;
  --btn-primary:#3B2F2A; --btn-primary-text:#FBF6EA;
  --v-green:#5E8C4A; --v-green-text:#3F6B2F;
  --v-yellow:#D49A1A; --v-yellow-text:#8A5A00;
  --v-red:#C4493A; --v-red-text:#A8372A;
  --font-display:"Pixelify Sans","Silkscreen",monospace;
  --font-ui:"Inter",system-ui,sans-serif;
  --font-mono:"JetBrains Mono",Consolas,monospace;
}
[data-mode="hacker"] {
  --bg-canvas:#06130A; --bg-surface:#0B2A12; --bg-sidebar:#07170C;
  --bg-code:#0B2A12; --text-code:#5CFF3A;
  --border:#1E6B12; --ink:#5CFF3A; --ink-soft:#2FB81B;
  --accent:#39FF14; --accent-strong:#39FF14; --accent-soft:#0B2A12;
  --btn-primary:#39FF14; --btn-primary-text:#06130A;
  --v-green:#39FF14; --v-green-text:#39FF14;
  --v-yellow:#FFD166; --v-yellow-text:#FFD166;
  --v-red:#FF6B6B; --v-red-text:#FF6B6B;
  --glow:0 0 12px rgba(57,255,20,0.45);
}
```

### 18.2 Suggested structure

```
/design    tokens.css  tokens.json
/assets
  mascot/ghost.svg  ghost.base.css  ghost.<state>.css  preview.html
  sounds/ (your files)  sounds/manifest.json  CREDITS.md
  icons/  fonts/
/ui
  components/  screens/ (chat, history, downloads, accounts, settings, onboarding)
  state/ (mode machine, task store, mascot machine, sound engine)
```

### 18.3 State you need to track
`mode` (chill/hacker/auto), `activePlan` (tasks + statuses), `runState` (idle/planning/awaiting-approval/running/stopped), `mascotState`, `safetyLevel`, `soundPrefs`. The mode switch, Task bar, mascot and sounds should all be driven by `runState` and task events, not by separate ad-hoc triggers.

---

## 19. Suggestions (UI)

Optional ideas, roughly in order of value:

Done in v0.3: the "1 held · 0 blocked" sub-line, the clickable Safety chip (opens Settings > Safety level), and the "Why?" toggle on YELLOW/RED plan steps. Dropped: a scripted demo mode (no demo content in the app).

1. **Command palette (`Ctrl+K`)** for new chat, switch mode, open Downloads, rewind last change, change safety level. Can reuse the slash-command registry.
2. **Pause as well as Stop**, and a per-task Skip button.
3. **Before/after diff for PATH and environment edits**, shown inside the task before approval.
4. **Log tools:** filter (errors only), copy log, export log in the Hacker terminal.
5. **OS notifications** when a task is held or the run finishes while the window is in the background.
6. **Keyboard shortcut cheat sheet** on `?`, and a collapsible sidebar for small windows.
7. **Keep the ghost lavender** against the warm palette (it's the one cool object); extend the same warm cream and caramel to the website so app and site match.
8. **Compact density option** (smaller spacing) for users who like dense UIs.
9. **Verdict legend** link near the Task bar ("What do GREEN / YELLOW / RED mean?") for first-time users.
10. **Bundle the fonts locally** (Inter, JetBrains Mono, Pixelify Sans) so the strict CSP can drop Google Fonts.
