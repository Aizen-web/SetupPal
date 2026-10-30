# SetupPal hackathon demo (3 minutes)

One idea: *you say what you want on your PC, SetupPal plans it, proves it is safe, runs it, verifies it,
and can undo it.* Pixel is the only character on screen.

## Before you go on stage

Do the reset checklist below, and keep the app in Chill mode on the Chats page.

## Script

| Time | You do | You say | What the audience sees |
|---|---|---|---|
| 0:00 | Open SetupPal. | "This is SetupPal. It sets up your computer from a sentence, and nothing runs that it has not checked first." | The window opens, Pixel waves. |
| 0:20 | Type: `set me up for Python data science` and send. | "I did not pick packages or commands." | The reply streams in, then a plan card appears. |
| 0:40 | Point at the plan card. Press **Why?** on a Yellow step. | "Every step gets a verdict: green is safe, yellow needs me, red never runs. This one has no upstream checksum, so it asks." | Step rows with verdict badges and plain-language reasons (typosquat, vulnerability, checksum, source domain). |
| 1:10 | Tick **Approve this step** on the Yellow step. Switch to **Hacker** mode. Press **Run**. | "I approve only the step I understand. Hacker mode shows the real run." | The terminal tab streams live step output. |
| 1:45 | Wait for the run to finish. | "Each step is verified after it runs. If anything fails it says so; no fake celebration." | Verification lines, then confetti only because everything passed. |
| 2:05 | Back in Chill mode, type: `install Minecraft Bedrock`. | "Some things can't be installed unattended, like Microsoft Store-only apps. SetupPal says so instead of pretending." | A manual setup card with Store steps and alternatives it can install. |
| 2:30 | Open **History**, press **Rewind** on the latest change. | "And everything it did can be undone." | The row flips to Rewind and the tool is removed. |
| 2:50 | Close. | "Zero trust: typed steps only, no shell, nothing red ever runs, and your keys live in Windows Credential Manager." | |

If the model or network is slow, say "the plan is being checked against OSV and the registries" while the
card builds; do not fill the time with anything that is not true.

## If something goes wrong

- **Provider error in chat:** the message shows the real error. Open Accounts, confirm the key says
  "Saved", and retry. Do not paste a key on screen.
- **No Yellow step appears:** pick another request, such as `set me up for web development`, or ask for a
  package without a published checksum.
- **A run halts early:** it is waiting for approval on a held step. Approve it and press the Approve and
  continue button.

## Reset checklist (between runs)

1. In **History**, rewind every change made during the last run, newest first.
2. Confirm the tools are gone: `winget list --id Python.Python.3.12`, and the venv folder
   `%USERPROFILE%\.venvs\python-ai` no longer exists.
3. Start a **New chat** so the plan card and terminal are empty (or use `/new`).
4. Open **Accounts** and check the active provider key says "Saved" (never show the value).
5. Set Settings back to Chill mode, sounds as wanted, reduced motion as wanted.
6. Close any second SetupPal window (a second launch only refocuses the first).
7. Disconnect from stage Wi-Fi only if you want to show the offline-first UI; the plan and chat need network.
