/**
 * SetupPal: Settings View (docs/UI.md Section 9.5)
 * Every control here is persisted and takes effect immediately.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { apiClient } from '../api/client.js';
import {
  getPrefs, setPrefs, resetPrefs, REPLY_LENGTHS, SAFETY_LEVELS, TEXT_SIZES,
} from '../state/prefs.js';
import { esc } from './html.js';
import { rememberOpener, confirmDialog } from './modal.js';

const MODE_LABELS = {
  auto: 'Auto (recommended)',
  'always-chill': 'Always Chill',
  'always-hacker': 'Always Hacker',
};
const BACKGROUND_LABELS = { off: 'Off', subtle: 'Subtle', lively: 'Lively' };

// Doctor results survive re-renders while the page stays open.
let doctor = { status: 'idle', data: null, error: null };

function row({ id, label, hint, control, labelFor = true }) {
  return `
    <div class="setting-row">
      <div class="setting-text">
        ${labelFor ? `<label class="setting-label" for="${id}" id="${id}-label">${label}</label>` : `<div class="setting-label" id="${id}-label">${label}</div>`}
        ${hint ? `<div class="setting-hint" id="${id}-hint">${hint}</div>` : ''}
      </div>
      ${control}
    </div>
  `;
}

function toggle(id, checked) {
  return `
    <span class="toggle">
      <input type="checkbox" role="switch" id="${id}" ${checked ? 'checked' : ''} aria-describedby="${id}-hint" />
      <span class="toggle-track" aria-hidden="true"></span>
    </span>
  `;
}

function select(id, options, current) {
  return `
    <select id="${id}" class="select" aria-describedby="${id}-hint">
      ${options.map(([value, label]) => `<option value="${esc(value)}" ${String(current) === String(value) ? 'selected' : ''}>${esc(label)}</option>`).join('')}
    </select>
  `;
}

function doctorHtml() {
  if (doctor.status === 'running') return '<p class="setting-hint">Scanning this PC…</p>';
  if (doctor.status === 'error') return `<p class="text-danger">Scan failed: ${esc(doctor.error)}</p>`;
  if (doctor.status !== 'done' || !doctor.data) {
    return '<p class="setting-hint">Run a scan to see the OS, GPU, and dev tools SetupPal detects on this PC.</p>';
  }
  const d = doctor.data;
  const tools = d.installed_tools || [];
  return `
    <div class="doctor-output">
      <div><strong>OS:</strong> ${esc(d.os_name)} ${d.os_version ? `(${esc(d.os_version)})` : ''} · <strong>Arch:</strong> ${esc(d.arch)}</div>
      <div><strong>GPU:</strong> ${d.gpu_detected ? esc(d.gpu_detected) : 'None detected'}</div>
      <div class="doctor-tools-title">Detected tools (${tools.length})</div>
      ${tools.length ? `<ul class="doctor-tools">${tools.map(t => `
        <li><strong>${esc(t.binary)}</strong>${t.version_output ? `: <code class="font-mono">${esc(t.version_output)}</code>` : ''}${t.path ? ` <span class="font-mono setting-hint">${esc(t.path)}</span>` : ''}</li>`).join('')}</ul>` : '<p class="setting-hint">No dev tools found on PATH.</p>'}
    </div>
  `;
}

function modelLine() {
  const health = apiClient.lastHealth;
  if (!health) return 'Backend offline. Model info shows up once it connects.';
  const provider = health.provider ? esc(health.provider) : 'Unknown provider';
  return `Model: <code class="font-mono">${health.model ? esc(health.model) : 'not reported'}</code> via ${provider} · API keys live on the Accounts page`;
}

export function renderSettingsView(container) {
  const state = store.getState();
  const settings = state.settings;
  const prefs = getPrefs();
  const safety = SAFETY_LEVELS[settings.safetyLevel] || SAFETY_LEVELS.strict;
  const prevScroll = container.querySelector('.page')?.scrollTop || 0;

  container.innerHTML = `
    <div class="main-content page">
      <div class="page-inner">
        <header class="page-header">
          <h1 class="page-title">Settings</h1>
          <p class="page-sub">Appearance, safety, chat memory, and sound.</p>
        </header>

        <div class="page-stack">
          <section class="card" aria-labelledby="settings-appearance-title">
            <h2 class="card-title" id="settings-appearance-title">Appearance</h2>
            ${row({
              id: 'setting-mode',
              label: 'Theme mode',
              hint: 'Auto switches to Hacker while a plan runs and holds each mode for at least 2 seconds, so it never flickers.',
              control: select('setting-mode', Object.entries(MODE_LABELS), state.modePreference),
            })}
            <div class="setting-row">
              <div class="setting-text">
                <div class="setting-label" id="setting-text-size-label">Text size</div>
                <div class="setting-hint" id="setting-text-size-hint">Scales chat, plans, and pages.</div>
              </div>
              <div class="seg" role="radiogroup" aria-labelledby="setting-text-size-label" aria-describedby="setting-text-size-hint">
                ${Object.entries(TEXT_SIZES).map(([key, v]) => `
                  <label class="seg-option">
                    <input type="radio" name="text-size" id="text-size-${key}" value="${key}" ${settings.textScale === key ? 'checked' : ''} />
                    <span>${v.label}</span>
                  </label>
                `).join('')}
              </div>
            </div>
            ${row({
              id: 'setting-reduce-motion',
              label: 'Reduce motion',
              hint: 'Calms Pixel and turns off sliding, bouncing, and confetti. Your system setting is always respected too.',
              control: toggle('setting-reduce-motion', settings.reduceMotion),
            })}
            ${row({
              id: 'setting-background',
              label: 'Ambient background',
              hint: 'A quiet theme-matched glow behind the workspace.',
              control: select('setting-background', Object.entries(BACKGROUND_LABELS), settings.backgroundIntensity),
            })}
          </section>

          <section class="card" aria-labelledby="settings-mascot-title">
            <h2 class="card-title" id="settings-mascot-title">Pixel</h2>
            ${row({
              id: 'setting-speech-bubbles',
              label: 'Speech bubbles',
              hint: 'Short flavor lines next to Pixel. Turn off for a quieter chat.',
              control: toggle('setting-speech-bubbles', settings.speechBubbles),
            })}
            ${row({
              id: 'btn-open-mascot-lab',
              label: 'Mascot Lab',
              hint: 'Preview every animation Pixel has.',
              labelFor: false,
              control: '<button id="btn-open-mascot-lab" class="btn-review btn-sm" type="button" aria-describedby="btn-open-mascot-lab-hint">Open lab</button>',
            })}
          </section>

          <section class="card" id="settings-safety" aria-labelledby="settings-safety-title">
            <h2 class="card-title" id="settings-safety-title">Safety level</h2>
            ${row({
              id: 'setting-safety',
              label: 'How much SetupPal asks first',
              hint: esc(safety.hint) + ' RED steps are always blocked.',
              control: select('setting-safety', Object.entries(SAFETY_LEVELS).map(([k, v]) => [k, k === 'strict' ? `${v.label} (Default)` : v.label]), settings.safetyLevel),
            })}
          </section>

          <section class="card" aria-labelledby="settings-chat-title">
            <h2 class="card-title" id="settings-chat-title">Chat &amp; memory</h2>
            <p class="card-sub">Older messages fold into short notes, so Pixel remembers what you said without resending the whole chat.</p>
            ${row({
              id: 'setting-memory',
              label: 'Recent messages kept in full',
              hint: 'More means better recall but more tokens.',
              control: select('setting-memory', [4, 6, 10, 16].map(n => [n, n === 6 ? `${n} (Default)` : String(n)]), prefs.memoryTurns),
            })}
            ${row({
              id: 'setting-reply-length',
              label: 'Reply length',
              hint: 'Upper limit on how long answers can get.',
              control: select('setting-reply-length', Object.entries(REPLY_LENGTHS).map(([k, v]) => [k, `${v.label} (${v.tokens})`]), prefs.replyLength),
            })}
            ${row({
              id: 'setting-pc-facts',
              label: 'Share PC facts with the AI',
              hint: 'OS, GPU, free disk, and installed dev tools. Needed for good recommendations.',
              control: toggle('setting-pc-facts', prefs.includePcFacts),
            })}
            <div class="setting-footer">
              <div class="setting-hint">${modelLine()}</div>
              <div class="btn-row">
                <button id="btn-clear-chats" class="btn-review btn-sm btn-danger" type="button">Clear all chats</button>
                <button id="btn-reset-prefs" class="btn-review btn-sm" type="button">Reset</button>
              </div>
            </div>
          </section>

          <section class="card" aria-labelledby="settings-doctor-title">
            <div class="card-head">
              <h2 class="card-title" id="settings-doctor-title">System scan</h2>
              <button id="btn-scan-doctor" class="btn-review btn-sm" type="button" ${doctor.status === 'running' ? 'disabled' : ''}>${doctor.status === 'done' ? 'Scan again' : 'Run scan'}</button>
            </div>
            <div id="doctor-profile-output" aria-live="polite">${doctorHtml()}</div>
          </section>

          <section class="card" aria-labelledby="settings-sound-title">
            <h2 class="card-title" id="settings-sound-title">Sound</h2>
            ${row({
              id: 'setting-sound',
              label: 'Sound effects',
              hint: 'Soft 8-bit blips, dings, and a small fanfare when a setup finishes.',
              control: toggle('setting-sound', settings.soundEnabled),
            })}
            ${row({
              id: 'setting-volume',
              label: 'Volume',
              hint: '',
              control: `<input type="range" id="setting-volume" class="range" min="0" max="1" step="0.05" value="${esc(settings.masterVolume)}" ${settings.soundEnabled ? '' : 'disabled'} />`,
            })}
          </section>
        </div>
      </div>
    </div>
  `;

  const page = container.querySelector('.page');
  if (page) page.scrollTop = prevScroll;

  const on = (id, event, handler) => {
    const el = container.querySelector(`#${id}`);
    if (el) el[event] = handler;
  };

  on('setting-mode', 'onchange', (e) => {
    store.setModePreference(e.target.value);
    sounds.play('ui.click');
  });
  container.querySelectorAll('input[name="text-size"]').forEach(input => {
    input.onchange = () => {
      store.updateSettings({ textScale: input.value });
      sounds.play('ui.click');
    };
  });
  on('setting-reduce-motion', 'onchange', (e) => store.updateSettings({ reduceMotion: e.target.checked }));
  on('setting-background', 'onchange', (e) => {
    store.updateSettings({ backgroundIntensity: e.target.value });
    sounds.play('ui.click');
  });
  on('setting-speech-bubbles', 'onchange', (e) => store.updateSettings({ speechBubbles: e.target.checked }));
  on('setting-safety', 'onchange', (e) => {
    store.updateSettings({ safetyLevel: e.target.value });
    sounds.play('ui.click');
  });

  on('setting-memory', 'onchange', (e) => {
    setPrefs({ memoryTurns: Number(e.target.value) });
    sounds.play('ui.click');
  });
  on('setting-reply-length', 'onchange', (e) => {
    setPrefs({ replyLength: e.target.value });
    sounds.play('ui.click');
  });
  on('setting-pc-facts', 'onchange', (e) => {
    setPrefs({ includePcFacts: e.target.checked });
    sounds.play('ui.click');
  });
  on('btn-reset-prefs', 'onclick', () => {
    resetPrefs();
    sounds.play('ui.click');
    renderSettingsView(container);
  });
  on('btn-clear-chats', 'onclick', async () => {
    const ok = await confirmDialog({
      title: 'Delete all saved chats?',
      message: 'This cannot be undone.',
      confirmLabel: 'Delete',
      danger: true,
    });
    if (!ok) return;
    ['setuppal_local_conversations_v2', 'setuppal_active_conv_id_v2'].forEach(k => localStorage.removeItem(k));
    sounds.play('ui.click');
    location.reload();
  });

  on('btn-scan-doctor', 'onclick', async () => {
    sounds.play('ui.click');
    doctor = { status: 'running', data: null, error: null };
    renderSettingsView(container);
    try {
      doctor = { status: 'done', data: await apiClient.getDoctor(), error: null };
      sounds.play('verdict.green');
    } catch (err) {
      doctor = { status: 'error', data: null, error: err?.message || 'The backend did not respond.' };
      sounds.play('error');
    }
    if (store.getState().activePage === 'settings') renderSettingsView(container);
  });

  on('setting-sound', 'onchange', (e) => {
    store.updateSettings({ soundEnabled: e.target.checked });
    if (e.target.checked) sounds.play('task.done');
  });
  on('setting-volume', 'oninput', (e) => store.updateSettings({ masterVolume: parseFloat(e.target.value) }));

  on('btn-open-mascot-lab', 'onclick', () => {
    rememberOpener();
    sounds.play('ui.click');
    store.toggleAnimationLab(true);
  });
}
