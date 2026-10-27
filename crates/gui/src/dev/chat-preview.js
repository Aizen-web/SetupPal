/**
 * Dev-only: renders every kind of assistant message in Chill and Hacker mode,
 * using the same renderMessage() the app uses. Open /chat-preview.html with `npm run dev`.
 * Add a scenario here whenever a new message shape is introduced.
 */
import '../styles/tokens.css';
import '../styles/base.css';
import '../styles/chat.css';
import { renderMessage } from '../components/chat-message.js';

const t = '06:34 PM';

const CLARIFY = [
  '**Setting up shaders and performance mods for Minecraft Java Edition.** Nothing has been changed yet.',
  '',
  '### System check',
  '- **Minecraft Java**: `C:\\Users\\there\\AppData\\Roaming\\.minecraft` (Status: Verified)',
  '- **Java runtime**: `25.0.1` (Status: Verified) — meets the Java 21+ this version needs',
  '- **Graphics**: `NVIDIA GeForce RTX 4060 Laptop GPU` (Status: Detected)',
  '- **Minecraft Bedrock**: Not installed (Status: Skipped) — these mods are for Java Edition',
  '',
  '### Your choices',
  '1. **Minecraft version**: `1.21.1` (Latest) | `1.20.4` | `1.19.4`',
  '2. **Mod engine**: `Iris Shaders + Sodium (Fabric)` (Recommended) — Usually the biggest FPS gain, modern shader packs, needs Fabric | `OptiFine (Classic)` — One-file install, simpler, but slower and fewer mod integrations',
  '3. **Install into**: `Isolated sandbox` (Recommended) — Staged in a separate profile, your .minecraft stays untouched | `Direct to .minecraft\\mods` — Writes straight into your live game folder',
  '',
  "Sources I'll check: modrinth.com · fabricmc.net · optifine.net",
  '',
  "Next: Pick your options and continue. I'll build a plan for you to approve.",
].join('\n');

const CLARIFY_ATTENTION = CLARIFY
  .replace('`25.0.1` (Status: Verified) — meets the Java 21+ this version needs', '`17.0.9` (Status: Attention) — this version needs Java 21+')
  .replace('**Minecraft Java**: `C:\\Users\\there\\AppData\\Roaming\\.minecraft`', '**Minecraft Java**: `C:\\Users\\there\\AppData\\Roaming\\PrismLauncher\\instances\\Fabric 1.21.1\\.minecraft`');

const PLAN = [
  '**Plan ready: Iris Shaders + Sodium (Fabric) for Minecraft 1.21.1.** 6 steps are listed below. Nothing has been changed yet.',
  '',
  '### Your setup',
  '- **Minecraft version**: `1.21.1`',
  '- **Mod engine**: `Iris Shaders + Sodium (Fabric)`',
  '- **Install into**: `Isolated sandbox` — `.\\.orchestrator-env\\minecraft-profile`',
  '- **Java runtime**: `25.0.1` (Status: Verified) — meets the Java 21+ this version needs',
  '',
  "Sources I'll check: modrinth.com · fabricmc.net",
  '',
  'Next: Review the steps, then choose **Approve safe steps**.',
].join('\n');

const NOKEY = [
  '⚠️ **No AI key set up yet**',
  '',
  'I can\'t write a custom plan for "make my terminal pretty" without an AI model. Add a key in **Settings**, or start from one of the offline recipes below.',
].join('\n');

const ERROR = [
  "⚠️ **Couldn't reach the AI model**",
  '',
  'I tried `meta/llama-3.1-70b-instruct` and got this error. Nothing has been changed.',
  '',
  '> `AI Provider returned HTTP 401 Unauthorized: {"status":401,"title":"Unauthorized","detail":"Authentication failed"}`',
  '',
  'Check your API key and connection in **Settings**, or use an offline recipe.',
].join('\n');

const ANSWER = [
  'Short answer: **yes**, OptiFine and Iris both run on Java Edition, but they are not compatible with each other.',
  '',
  'Things worth knowing:',
  '',
  '- OptiFine is a single `.jar` that works on its own.',
  '- Iris needs the Fabric loader and Sodium.',
  '- Run `java -version` to check your runtime.',
  '',
  '```powershell',
  'java -version',
  '```',
  '',
  '| Option | Needs Fabric | Shader packs |',
  '|---|---|---|',
  '| OptiFine | No | Yes |',
  '| Iris + Sodium | Yes | Yes |',
].join('\n');

// What the old build produced (the screenshot): bullet soup + inline questions.
const LEGACY = [
  '### Section 1: Scope & Context',
  'Configuring Minecraft performance optimization and visual shader mods on Windows 11.',
  '',
  '### Section 2: Host Environment (Key-Value Grid)',
  '- **Runtime**: `Java Runtime` (Status: Missing on PATH)',
  '- **Target Hardware**: `NVIDIA GeForce RTX 4060 Laptop GPU`',
  '- **Active Path**: `.\\.orchestrator-env\\minecraft-profile`',
  '- **Policy**: `Sandboxed`',
  '',
  '### Section 3: Configuration Choices & Next Steps',
  '1. **Target Version**: `1.21.1` (Latest Stable) | `1.20.4` (LTS Compatibility) | `1.19.4`',
  '2. **Engine / Tool Preference**: `Iris Shaders + Sodium (Fabric)` (Recommended for RTX/GPU) vs `OptiFine (Classic)`',
  '',
  '`Next: Select your target version and approve staged sandbox execution →`',
].join('\n');

const SOUP = 'Minecraft Java Edition detected. • **Java Runtime**: ✅ Installed (`25.0.1`) • **Graphics Hardware**: `NVIDIA GeForce RTX 4060 Laptop GPU` • **Zero-Trust Safety Policy**: Strict Planning Active';

const user = (text) => ({ id: 'u1', sender: 'user', text, time: t });
const bot = (id, text, options) => ({ id, sender: 'assistant', text, time: t, options });

const SCENARIOS = {
  'Clarification': [user('I want to download minecraft mods like optifine.'), bot('a_clarify', CLARIFY)],
  'Clarification (attention)': [user('I want to download minecraft mods like optifine.'), bot('a_clarify2', CLARIFY_ATTENTION)],
  'Plan ready': [user('Minecraft version: 1.21.1 · Mod engine: Iris Shaders + Sodium (Fabric) · Install into: Isolated sandbox'), bot('a_plan', PLAN)],
  'No AI key': [user('make my terminal pretty'), bot('a_nokey', NOKEY, [
    { label: 'Open Settings', value: 'open_settings' },
    { label: 'Minecraft shaders and mods', value: 'x' },
    { label: 'Python data science', value: 'y' },
    { label: 'C++ toolchain', value: 'z' },
  ])],
  'Error': [user('set up python'), bot('a_err', ERROR, [{ label: 'Open Settings', value: 'open_settings' }])],
  'Plain answer': [user('is optifine compatible with iris?'), bot('a_ans', ANSWER)],
  'Answered form (locked)': [
    user('I want to download minecraft mods like optifine.'),
    bot('a_old', CLARIFY),
    user('Minecraft version: 1.21.1 · Mod engine: Iris Shaders + Sodium (Fabric) · Install into: Isolated sandbox'),
    bot('a_new', PLAN),
  ],
  'Old build output': [bot('a_legacy', LEGACY)],
  'Bullet soup': [bot('a_soup', SOUP)],
};

const ui = { selections: {}, collapsed: new Set() };
const state = { scenario: 'Clarification', mode: 'chill', width: 'full' };

const bar = document.getElementById('pv-bar');
const thread = document.getElementById('pv-thread');
const widthEl = document.getElementById('pv-width');

function render() {
  document.documentElement.dataset.mode = state.mode;
  widthEl.style.maxWidth = state.width === 'narrow' ? '480px' : '';

  const msgs = SCENARIOS[state.scenario];
  thread.innerHTML = msgs.map((m, i) => renderMessage(m, {
    isLast: i === msgs.length - 1,
    isNew: false,
    activePet: 'ghost',
    ui,
  })).join('');

  bar.innerHTML = [
    ...Object.keys(SCENARIOS).map(k => `<button data-s="${k}" class="${k === state.scenario ? 'on' : ''}">${k}</button>`),
    '<span class="pv-sep"></span>',
    `<button data-m="chill" class="${state.mode === 'chill' ? 'on' : ''}">Chill</button>`,
    `<button data-m="hacker" class="${state.mode === 'hacker' ? 'on' : ''}">Hacker</button>`,
    '<span class="pv-sep"></span>',
    `<button data-width="full" class="${state.width === 'full' ? 'on' : ''}">Full</button>`,
    `<button data-width="narrow" class="${state.width === 'narrow' ? 'on' : ''}">Narrow</button>`,
  ].join('');

  bar.querySelectorAll('button').forEach(b => {
    b.onclick = () => {
      if (b.dataset.s) state.scenario = b.dataset.s;
      if (b.dataset.m) state.mode = b.dataset.m;
      if (b.dataset.width) state.width = b.dataset.width;
      render();
    };
  });

  // Same behaviours as chat-view.js, minus the app plumbing
  thread.querySelectorAll('.choice-input').forEach(input => {
    input.onchange = () => {
      ui.selections[`${input.dataset.form}:${input.dataset.gi}`] = Number(input.dataset.oi);
      const form = input.closest('.chat-choice-form');
      const summary = form.querySelector('.choice-summary');
      if (summary) summary.textContent = [...form.querySelectorAll('.choice-input:checked')].map(i => i.value).join(' · ');
    };
  });
  thread.querySelectorAll('details.chat-sys').forEach(d => {
    d.addEventListener('toggle', () => {
      const key = d.getAttribute('data-key');
      if (d.open) ui.collapsed.delete(key); else ui.collapsed.add(key);
    });
  });
  thread.querySelectorAll('.chat-continue-btn').forEach(btn => {
    btn.onclick = () => {
      const form = btn.closest('.chat-choice-form');
      const parts = [...form.querySelectorAll('.choice-group[data-name]')].map(g => {
        const c = g.querySelector('.choice-input:checked');
        return c ? `${g.dataset.name}: ${c.value}` : '';
      }).filter(Boolean);
      alert('Would send:\n\n' + parts.join(' · '));
    };
  });
}

render();
