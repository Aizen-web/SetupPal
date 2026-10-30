# SetupPal Security Architecture & Safety Guide

## Overview

SetupPal helps people install and configure software safely. Because an AI planner can be misled by search poisoning, hallucinated package names, or copycat projects, SetupPal never gives the model unrestricted access to the machine. A deterministic, multi-layer safety system sits between the AI and the OS: actions are strictly typed, inspected against security signals, executed with the lowest practical permissions, verified after completion, and recorded in an undoable ledger.

## Operational Boundary: What SetupPal Will Install

SetupPal installs **anything that passes two gates**. It is not limited to developer tools.

1. **Unattended CLI install and verification:** The software can be installed and verified from the command line with no interactive prompts (for example `winget install --silent`, `pip`, `npm`, `cargo`, official MSI/EXE silent flags such as `/quiet`, or Modrinth/Fabric through its CLI installer). Verification uses a fresh-process check such as a version command, `winget list`, or `pip show`.
2. **Safety Engine:** Every step passes the checks in [The Protection Layers](#the-protection-layers) and receives a Green or approved Yellow rating. Red steps are never run.

**Manual-only software.** Anything that fails the first gate is not installed. SetupPal replies with a **Manual setup guide** that explains the limit, gives step-by-step instructions, and lists similar things SetupPal *can* install.

**Trusted through winget means installable.** A winget package from the community repository has a reviewed manifest, a pinned installer URL, and a SHA-256 that winget checks before running it. If its installer type runs unattended (`msi`, `msix`, `wix`, `inno`, `nullsoft`, `burn`, `portable`, `zip`, or `exe` with the silent switches the community repo requires), SetupPal treats it as installable. That includes game launchers and apps that ask for a sign-in after install—Steam, Epic Games, Roblox, Discord, LMMS, and similar. Sign-in happens later in the app and does not affect install or verification (`winget list --id <id> --exact`).

The gate judges each package by its winget manifest, never by a name list or by the AI model’s opinion. These stay manual-only:

1. **Microsoft Store-only products** (Store IDs such as `9NBLGGH2JHXJ` for Minecraft Bedrock, or installer type `msstore`): they need a Store account and cannot be hash-checked against a static digest.
2. **Vendor download pages** with no package-manager entry (for example most GPU drivers).
3. **Installers with no unattended mode**, and anything the Safety Engine rates Red.

This is a per-app decision. Minecraft Bedrock is manual-only; the Minecraft Launcher (`Mojang.MinecraftLauncher`) and a Fabric mod setup for Minecraft Java can be installed.

---

## How a Typical Install Works

1. **User states a goal** in plain English (for example *"set up Python"* or *"install Minecraft mods"*).
2. **SetupPal builds a plan card.** The AI may suggest steps, but nothing runs yet. The card lists every step, source, and risk rating. Manual-only requests get a Manual setup guide instead (see [Operational Boundary](#operational-boundary-what-setuppal-will-install)).
3. **One-click approval** when every step is Green. Yellow steps pause for explicit consent.
4. **Narrated execution** with live progress. Pixel narrates mood only; facts stay in the activity feed and terminal.
5. **Post-install verification** in a clean process (for example `python --version`). Outcomes are reported honestly.
6. **One-click Undo (Rewind)** from History for SetupPal-managed changes.

---

## Security vs. Ease of Use

- **Ask by risk, not by step:**
  - **Green:** verified safe; auto-approved within the plan; logged in the feed.
  - **Yellow:** unverified, unusual, or higher risk; one confirmation with a plain-language reason.
  - **Red:** blocked immediately. There is no “Run Anyway” for Red; SetupPal explains the threat and points to a safer alternative when one exists.
- **Approve the plan once** when all steps are Green. Only a Yellow step pauses the sequence, and only for that step.
- **Narrow trust accumulation:** optional “Don’t ask about this kind of thing again” is scoped to a category (for example verified winget packages), never global. Yellow and Red always interrupt.
- **Undo is the primary safety net:** tracked, reversible actions reduce upfront friction without hiding risk.

---

## The Traffic-Light System

Every action gets a deterministic risk rating (score 0–100):

| Rating | Score Range | What It Means | What the User Sees | User Action Required |
| :--- | :--- | :--- | :--- | :--- |
| **Green** | 0 – 29 | Verified official source, trusted publisher, clean vulnerability record, valid checksum. | Green badge with verified source links (e.g. `Getting this from python.org`). | None. Auto-approved within the plan. |
| **Yellow** | 30 – 69 | Unverified site, newly published package, low download count, missing checksum, or admin requirement. | Yellow warning with a plain-English reason and source domain. | One-time approval for this step. |
| **Red** | 70 – 100 | Known CVE, typosquat / copycat, spoofed domain, or prohibited command. | Red block with risk explanation and a safe alternative when available. | Hard block. No override. |

---

## Source Transparency

- Plain-language label with a clickable link to the live destination (e.g. `Getting this from python.org` → `https://www.python.org`).
- The displayed domain is always the real network host—never a friendly mask.
- Domains outside the official allowlist or verified consensus show the raw host and are flagged Yellow or Red.

---

## Plain-Language UI Translations

| Technical Finding / Signal | Plain-English UI Translation | User-Facing Explanation |
| :--- | :--- | :--- |
| SHA-256 digest match | Verified source | "File fingerprint matches the official release." |
| Typosquat match (edit distance 1–2) | Lookalike warning | "This looks like an unofficial copy of the popular package `<name>`." |
| Active OSV CVE vulnerability | Known security flaw | "This version has an unresolved security issue. SetupPal recommends version `<version>` instead." |
| Domain in verified allowlist | Official source | "Downloaded directly from the official developer website." |
| Domain not in allowlist / unverified | Unverified website | "This download comes from a website SetupPal could not independently verify." |
| Package age < 30 days | Newly published software | "This package was released recently (less than 30 days ago)." |
| Download count < 1,000 | Low community adoption | "Very few people have downloaded this package so far." |
| Verification API timeout / offline | Verification unavailable | "Couldn't reach our safety checks right now. Proceed with caution?" |
| Missing upstream checksum | Unchecked file integrity | "The publisher did not provide a checksum to verify file integrity." |
| Requires UAC / Administrator | Administrator access needed | "This tool requires administrator approval to install system-level components." |

---

## The Protection Layers

### Layer 1: Closed Action Schema

- **Threat:** hallucinations, prompt injection, dangerous shell (`curl … \| sh`, `rm -rf`, drive formatting).
- **How:** The AI proposes only structured `Step` actions (`InstallPackage`, `DownloadArtifact`, `VerifyHash`, `AddToUserPath`, `CreateIsolatedEnv`, `RunVerifyCommand`). Arguments cannot contain shell chaining characters and are passed as process argv—no `cmd.exe` / `powershell.exe` wrapper.
- **Limit:** Does not inspect internals of binaries launched by an allowed package manager.

### Layer 2: Deterministic Safety Radar

- **Threat:** supply-chain malware, copycats, known CVEs, SEO-poisoned domains.
- **How:** Hard policy (prohibited commands, metacharacters, sensitive paths); typosquat radar (Levenshtein); OSV.dev; optional Tavily consensus; registry metadata (age, downloads, scripts).
- **Limit:** Brand-new malware with clean metadata and no typosquat signal may not be caught.

### Layer 3: User-Scope Execution

- **Threat:** system-wide compromise and OS corruption.
- **How:** Prefer user-scope flags (`winget --scope user`), project `.venv` for Python, local Node installs. No permission to rewrite `System32` or global Program Files by default.
- **Limit:** User-scope code still has the logged-in user’s file and network rights.

### Layer 4: Hash & Checksum Verification

- **Threat:** MITM, truncated downloads, corrupted releases.
- **How:** Artifacts must match upstream SHA-256 before use; mismatch → Red halt.
- **Limit:** If the upstream account is compromised and both file and checksum are replaced, hash checks alone cannot detect it.

### Layer 5: Transaction Ledger & 1-Click Undo

- **Threat:** orphaned files, polluted PATH, user remorse.
- **How:** Append-only `.orchestrator_state.jsonl` with inverse rollback data. **Rewind** runs those actions in reverse.
- **Limit:** Only SetupPal-managed paths and packages are removed. Runtime files the app creates elsewhere may remain.

---

## Handling Admin-Required Installs

When an install still passes the unattended gate and Safety Engine but needs elevation:

1. Never bundled as Green / silent auto-approve.
2. Always Yellow with a clear rationale.
3. Windows shows the normal UAC prompt—SetupPal does not bypass it.

---

## Failure Modes and Defaults

- **Verification outage:** fail closed to **Yellow**, never silent Green.
- **Ambiguous packages:** Yellow; user picks the authentic publisher.
- **Checksum mismatch:** **Red**; file is not executed.
- **Post-install verify failure:** honest failure, Undo offered—no fake success.
- **No unattended path:** Manual setup guide; no install attempt.

---

## What SetupPal Does NOT Protect Against

1. Malicious code running with the user’s permissions.
2. Unreported / zero-day packages with clean metadata.
3. Compromised publisher accounts on official registries.
4. Bugs or incompatibility in third-party software.
5. User approval of Yellow unverified steps.
6. Steps the user performs themselves from a Manual setup guide (outside Safety Engine, verify, and Undo).

---

## Technical Appendix

### 1. Risk Scoring Engine & Thresholds

Risk scores are integers `0`–`100`. Plan rating = maximum step rating (`Green < Yellow < Red`).

```
Risk Scores:
  0 ------------ 29 | 30 ------------------------ 69 | 70 ---------------- 100
       GREEN        |              YELLOW             |            RED
  (Auto-approvable) |     (Explicit consent needed)   |      (Hard blocked)
```

| Signal / Evaluation Condition | Assigned Score | Resulting Rating | Implementation Module |
| :--- | :--- | :--- | :--- |
| Hard policy violation (shell metacharacters, prohibited commands, credential store access) | `100` | **Red** | `crates/core/src/safety/policy.rs` |
| SHA-256 verification mismatch | `100` | **Red** | `crates/core/src/safety/hash.rs` |
| Tavily search consensus confirms spoofed / lookalike domain | `100` | **Red** | `crates/core/src/safety/tavily.rs` |
| Typosquatting detected (Levenshtein distance $1 \le d \le 2$ from top package) | `95` | **Red** | `crates/core/src/safety/typosquat.rs` |
| Download URL domain not in allowlist and unverified by search consensus | `95` | **Red** | `crates/core/src/safety/policy.rs` |
| Active OSV.dev CVE record found | `90` | **Red** | `crates/core/src/safety/osv.rs` |
| Critical install script flag (base64 payloads, direct socket/network calls) | `90` | **Red** | `crates/core/src/safety/policy.rs` |
| Non-critical install script warning | `55` | **Yellow** | `crates/core/src/safety/policy.rs` |
| Upstream registry metadata verification failure / network error | `50` | **Yellow** | `crates/core/src/safety/mod.rs` |
| Young package warning (published $< 30$ days ago) | `45` | **Yellow** | `crates/core/src/safety/mod.rs` |
| Low adoption warning ($< 1,000$ recorded downloads) | `45` | **Yellow** | `crates/core/src/safety/mod.rs` |
| Missing upstream SHA-256 digest | `40` | **Yellow** | `crates/core/src/safety/mod.rs` |
| All checks passed cleanly | `0` | **Green** | `crates/core/src/safety/mod.rs` |

### 2. External Data Sources & APIs

- **OSV.dev REST API (`https://api.osv.dev/v1/query`):** vulnerability data for ecosystems SetupPal queries (PyPI and npm in this tree).
- **Tavily AI Search API (`https://api.tavily.com/search`):** optional web consensus for authentic distribution domains.
- **Package registry metadata:** winget local manifests; PyPI, npm, Modrinth API v2; GitHub Releases.

### 3. Core Codebase Modules

```
crates/core/src/
├── types.rs           # Closed Action Schema (Step enum, AllowedBinary, Plan, Ecosystem)
├── safety/
│   ├── mod.rs         # SafetyEngine: orchestrator for all safety evaluations and scoring
│   ├── policy.rs      # Allowlist domains, shell metacharacter filters, destructive pattern matchers
│   ├── typosquat.rs   # Levenshtein distance matching against popular package corpuses
│   ├── osv.rs         # OSV.dev client and CVE vulnerability models
│   ├── tavily.rs      # Web consensus engine detecting SEO poisoning and lookalikes
│   └── hash.rs        # Streaming SHA-256 calculation and verification
├── state/
│   └── mod.rs         # Append-only ledger (.orchestrator_state.jsonl) and rollback engine
├── verifier/
│   └── mod.rs         # Fresh-process environment and binary version verification
└── executor/
    └── mod.rs         # Step executor enforcing user-scope flags and argument safety
```

### 4. Append-Only Transaction Ledger Format

All state modifications are appended to `.orchestrator_state.jsonl` as structured JSON objects:

```json
{
  "tx_id": "tx-8f3a1b02-5c74-4b2e",
  "timestamp": "2026-10-26T18:30:00Z",
  "plan_id": "plan-python-312",
  "goal": "set up Python",
  "step_id": "step-1",
  "action_summary": "Installed Python 3.12 via winget (user scope)",
  "step": {
    "action": "install_package",
    "params": {
      "ecosystem": "winget",
      "package_id": "Python.Python.3.12",
      "version": "3.12.2",
      "source_adapter": "winget"
    }
  },
  "dry_run": false,
  "changes_made": [
    "Installed package Python.Python.3.12 into user profile",
    "Registered user environment PATH entry"
  ],
  "rollback_hint": "winget uninstall Python.Python.3.12 --silent",
  "rollback_data": {
    "type": "package_install",
    "data": {
      "ecosystem": "winget",
      "package_id": "Python.Python.3.12",
      "uninstall_command": "winget uninstall Python.Python.3.12 --silent"
    }
  }
}
```

### 5. Deterministic Rollback Mappings

| Action Type | Ledger Data Type (`RollbackData`) | Rollback Mechanism Executed on Undo |
| :--- | :--- | :--- |
| `InstallPackage` | `PackageInstall` | Spawns registered silent uninstaller (e.g., `winget uninstall <ID> --silent` or `pip uninstall -y <ID>`). |
| `DownloadArtifact` | `FileDownload` | Deletes the downloaded artifact file directly from the filesystem (`std::fs::remove_file`). |
| `CreateIsolatedEnv` | `IsolatedEnv` | Recursively removes the isolated directory (`std::fs::remove_dir_all`), such as `.venv/`. |
| `AddToUserPath` | `PathBackup` | Replaces the current `HKCU\Environment\Path` registry key with the preserved snapshot. |
| `RunVerifyCommand` | *None* | Read-only check; records no rollback action. |
| `VerifyHash` | *None* | Read-only check; records no rollback action. |
