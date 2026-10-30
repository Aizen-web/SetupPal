# SetupPal demo script (~3 minutes)

One idea: *you say what you want on your PC; SetupPal plans it with NVIDIA Nemotron on Nebius Token Factory, proves it is safe, runs only what you approve, verifies it, and can undo it.* Pixel is the only character on screen.

**Hackathon requirement:** the public YouTube video must be **3 minutes or shorter** and must clearly show Nebius Token Factory + NVIDIA Nemotron in use (not only a local recipe with the network off).

## Before you present or record

Complete the [reset checklist](#reset-checklist-between-runs). Keep the app in Chill mode on the Chats page. Confirm the active provider is **Nebius** and the key shows **Saved** in Accounts—never display the value.

Optional on-camera proof (first 20–30 seconds): open Accounts long enough to show “Nebius” / Token Factory and **Saved**, then return to chat. Do not scroll into any secret fields.

## Proof points the video must hit

1. **Nebius + Nemotron:** Accounts shows Nebius Saved, or chat streams a live model reply (not a canned offline string).
2. **Plan card:** a typed plan appears after a natural-language request.
3. **Safety Engine:** Green/Yellow ratings and a Why? reason on at least one step.
4. **Approve → run → verify:** only approved steps run; verification is honest.
5. **Boundary or Rewind:** Manual setup guide (Store-only) and/or History → Rewind.

## Script

| Time | You do | You say | What the audience sees |
|---|---|---|---|
| 0:00 | Open SetupPal. Briefly open **Accounts** if needed to show Nebius **Saved**, then Chats. | "This is SetupPal. It sets up your Windows PC from a sentence. Planning runs on Nebius Token Factory with NVIDIA Nemotron—and nothing runs that SetupPal has not checked first." | Window opens, Pixel waves; Accounts shows Nebius Saved (no key value). |
| 0:25 | Type: `set me up for Python data science` and send. | "I did not pick packages or commands. Nemotron on Token Factory turns the sentence into a plan." | Reply streams in (live AI), then a plan card appears. |
| 0:50 | Point at the plan card. Press **Why?** on a Yellow step. | "Every step gets a verdict: green is safe, yellow needs me, red never runs. The model proposed this; Rust rated it." | Step rows with verdict badges and plain-language reasons (typosquat, vulnerability, checksum, source domain). |
| 1:15 | Tick **Approve this step** on the Yellow step. Switch to **Hacker** mode. Press **Run**. | "I approve only the step I understand. Hacker mode shows the real run—closed actions, no free-form shell." | Terminal tab streams live step output. |
| 1:50 | Wait for the run to finish. | "Each step is verified after it runs. If anything fails it says so; no fake celebration." | Verification lines, then confetti only because everything passed. |
| 2:10 | Back in Chill mode, type: `install Minecraft Bedrock`. | "Some things cannot be installed unattended, like Microsoft Store-only apps. SetupPal says so instead of pretending." | A manual setup card with Store steps and alternatives it can install. |
| 2:30 | Open **History**, press **Rewind** on the latest change. | "And everything it did can be undone." | The row flips to Rewound and the tool is removed. |
| 2:50 | Close. | "Zero trust: Nemotron proposes on Nebius, typed steps only, nothing red ever runs, keys stay in Windows Credential Manager." | |

If the model or network is slow, say that the plan is being checked against OSV and the registries while the card builds. Do not invent status that is not true.

## If something goes wrong

- **Provider error in chat:** the message shows the real error. Open Accounts, confirm Nebius shows "Saved", and retry. Do not paste a key on screen.
- **No Yellow step appears:** try another request, such as `set me up for web development`, or ask for a package without a published checksum. Still show Green ratings and Why? if available.
- **A run halts early:** it is waiting for approval on a held step. Approve it and press Approve and continue.
- **Need a shorter cut:** drop Minecraft Bedrock *or* Rewind—not both—and keep Nebius/Nemotron + plan + safety + one verify.

## Recording and publishing (YouTube)

1. Record 1080p (or higher) with system audio optional; keep the window readable.
2. Stay at or under **3:00**. Lead with Nebius/Nemotron; do not spend the first minute on setup slides.
3. Upload as a **public** (or unlisted-if-allowed-by-Devpost; prefer public) YouTube video.
4. Title suggestion: `SetupPal — Nebius Token Factory + NVIDIA Nemotron (hackathon demo)`.
5. Description should mention: SetupPal, Nebius Token Factory, Nemotron Lightning (default), GitHub `https://github.com/Aizen-web/SetupPal`.
6. Paste the real URL into Devpost and optionally into [SUBMISSION.md](../SUBMISSION.md) / [README.md](../README.md). **Do not** commit a fake or “TBD” video link.

## Reset checklist (between runs)

1. In **History**, rewind every change made during the last run, newest first.
2. Confirm the tools are gone: for example `winget list --id Python.Python.3.12`, and the venv folder `%USERPROFILE%\.venvs\python-ai` no longer exists.
3. Start a **New chat** so the plan card and terminal are empty (or use `/new`).
4. Open **Accounts** and check Nebius shows "Saved" (never show the value).
5. Set Settings back to Chill mode; restore sounds and reduced-motion preferences as needed.
6. Close any second SetupPal window (a second launch only refocuses the first).
7. Disconnect from stage Wi-Fi only if you want to show the offline-first UI; planning and chat still need network for the Nebius demo.
