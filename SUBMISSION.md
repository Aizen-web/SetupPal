# SetupPal — Devpost submission pack

Ready-to-paste copy for the [Nebius × NVIDIA Global AI Hackathon](https://nebiusglobalaihackathon.devpost.com/).
Track: **Best Apps and Agents**.
Built during the submission period (not a prior-existing product submission).

Public repository: https://github.com/Aizen-web/SetupPal
License: MIT ([LICENSE](LICENSE))

Do **not** paste placeholder URLs into Devpost. Add the real YouTube demo URL and the working test-build URL (GitHub Release installer) only after they exist. See [Final submission checklist](#final-submission-checklist).

---

## Category

**Best Apps and Agents Track**

SetupPal is a Windows desktop app people would actually use: say what you want on your PC, get a typed install plan from NVIDIA Nemotron on Nebius Token Factory, approve only what the Safety Engine allows, verify the result, and Rewind if needed.

---

## Inspiration

AI chat can suggest installs, but trusting a model with your machine is a bad idea. Package names get hallucinated, typosquats slip through, and “just run this command” is not a product. We wanted a complete experience where open models on Nebius do the language work, and deterministic Rust decides what is allowed to touch the OS—with the user still in the loop.

---

## What it does

SetupPal turns a plain-English goal into a **plan card** (from a YAML recipe or the pinned LLM), rates every step Green / Yellow / Red, runs only approved closed-schema actions (winget, npm, pip, cargo, and similar—no free-form shell), verifies afterward, and records changes in a ledger you can **Rewind**.

If something cannot be installed unattended (for example Microsoft Store-only apps), SetupPal gives a **Manual setup guide** instead of pretending.

---

## How we built it

- **Core** (`installer_core` in Rust): scan, recipes, LLM client, Safety Engine, adapters, executor, verifier, healer, JSONL state ledger.
- **Desktop**: Tauri v2 + local Axum API (random localhost port, per-launch bearer token) + vanilla JS / Vite UI, Pixel mascot, Chill / Hacker modes.
- **CLI**: same core for `doctor`, `plan`, `install`, `undo`.
- **AI path**: streaming chat + structured `setuppal-intent` → plan; production calls go to Nebius Token Factory with a pinned Nemotron model ID.
- **Gates**: installability (unattended CLI + winget manifest types) and Safety Engine (traffic lights, typosquat, OSV, optional Tavily consensus).

---

## How we used Nebius Token Factory and NVIDIA Nemotron

| Item | Value |
| --- | --- |
| Service | Nebius Token Factory (`https://api.tokenfactory.nebius.com/v1`) |
| Models | Lightning `nvidia/Nemotron-3_5-Lightning`, Super (default) `nvidia/nemotron-3-super-120b-a12b`, Ultra `nvidia/Nemotron-3-Ultra-550b-a55b` (Nebius catalog IDs in `crates/core/src/config.rs`) |
| Role | Chat streaming and structured install planning / intents |
| Not used for | Execution, privilege decisions, or Safety Engine verdicts |

Token Factory accelerated the workflow with an OpenAI-compatible chat-completions API, so the app can offer three Nemotron tiers, stream replies for a responsive UI, and keep BYOK keys in Windows Credential Manager. NVIDIA NIM is not used.

Optional **Tavily** is used only for Safety Engine source-domain consensus—not as general web search for chat.

---

## Challenges we ran into

- Keeping the model useful without ever giving it a shell: closed `Step` schema, plan card as source of truth, Red hard-blocks.
- Distinguishing “has a GUI” from “cannot install unattended”—winget silent types vs Store-only / vendor pages.
- Making small/fast models emit reliable structured intents while still streaming a natural reply.
- Honest release claims: key-free CI vs live corpus and clean-VM Rewind gates that need a real Windows environment and keys.

---

## Accomplishments that we're proud of

- A coherent product loop: chat → plan → safety → approve → verify → Rewind.
- Nebius-only production path with three catalog Nemotron models (Super default) and a clear Settings selector.
- Deterministic Safety Engine and installability gate that do not defer authority to the LLM.
- Contributor docs, MIT license, Windows CI (`scripts/check.ps1`), and NSIS packaging with tagged GitHub Releases.

---

## What we learned

- Open models on Token Factory are strong at turning goals into plans; security still belongs in deterministic code.
- Judges and users need the same story in the README, the plan card, and the demo—not three different pitches.
- BYOK + Credential Manager is the right default for a local agent that can change the machine.

---

## What's next for SetupPal

- Fill and publish clean-VM install/Rewind evidence as releases mature.
- Broader adapter coverage and tighter timeout handling for long installs.
- Accessibility audit and richer History export—without weakening the zero-trust boundary.

---

## Built with

- Nebius Token Factory
- NVIDIA Nemotron (Lightning / Super / Ultra via Nebius Token Factory)
- Rust, Tokio, Axum
- Tauri v2, WebView2, Vite
- winget / npm / pip / cargo adapters
- OSV; optional Tavily
- Windows Credential Manager

---

## Feedback on Nebius Token Factory, AI Cloud, and NVIDIA tools

**Token Factory:** OpenAI-compatible `/v1/chat/completions` made it straightforward to pin one Nemotron model for both streaming chat and structured planning. Clear base URL and key-based auth fit a local desktop BYOK product well.

**Nemotron on Nebius:** Fast enough for interactive plan cards when prompts stay short and schema-constrained. Pinning a single model ID (no live model list) kept the product predictable for demos and CI.

**Wish list:** Documented “hackathon judge” or short-lived sandbox keys would lower friction for Windows-only desktop evals; clearer guidance on which Nemotron IDs are currently available on Token Factory would reduce model-ID drift risk.

---

## Potential impact (judging)

SetupPal addresses a real audience: anyone who wants AI help setting up a Windows PC without handing the model a shell. The demo shows a specific loop—plan, rate, approve, verify, undo—not a generic chatbot. The creative angle is **proposal ≠ authority**: Nebius-hosted Nemotron proposes; Rust gates authorize.

---

## Prior work disclosure

SetupPal was **built during the submission period**. No “significant updates to a pre-existing project” disclosure is required for this submission.

---

## Final submission checklist

Complete on Devpost (and update this file only with **real** URLs when you have them):

1. [ ] Category: Best Apps and Agents
2. [ ] Project description pasted from sections above
3. [ ] Public repo URL: `https://github.com/Aizen-web/SetupPal` (MIT license visible)
4. [ ] Working demo / test build: latest GitHub Release NSIS installer
   (`https://github.com/Aizen-web/SetupPal/releases`)
5. [ ] Demo video: public YouTube, ≤ 3 minutes, shows Nebius Token Factory + NVIDIA Nemotron in use
   (record with [docs/DEMO.md](docs/DEMO.md); paste URL into Devpost when published)
6. [ ] Feedback on Token Factory / Nemotron included
7. [ ] Built-during-submission-period statement included
8. [ ] No API keys, secrets, or placeholder media in the repo or Devpost fields

### After you publish the video

Optionally add one line under **Docs** in [README.md](README.md):

```markdown
- Demo video: <your public YouTube URL>
```
