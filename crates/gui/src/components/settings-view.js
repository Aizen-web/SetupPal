/**
 * SetupPal: Settings View (ui.md Section 9.5)
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { apiClient } from '../api/client.js';
import { icon } from './icon.js';

export function renderSettingsView(container) {
  const state = store.getState();
  const settings = state.settings;

  container.innerHTML = `
    <div class="main-content" style="padding: 24px 32px; overflow-y: auto;">
      <div style="margin-bottom: 24px;">
        <h1 class="font-pixel" style="font-size: 24px; color: var(--ink);">Settings</h1>
        <div style="font-size: 13px; color: var(--ink-soft); margin-top: 4px;">
          Configure appearance, safety gates, audio engine, and mascot behavior.
        </div>
      </div>

      <div style="display: flex; flex-direction: column; gap: 20px; max-width: 680px;">
        <!-- Appearance Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin-bottom: 14px;">Appearance & Modes</h2>
          
          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0; border-bottom: 1px solid var(--border-subtle);">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Theme Mode</div>
              <div style="font-size: 12px; color: var(--ink-soft);">Auto-switches to Hacker mode during tool and task execution</div>
            </div>
            <select id="select-mode" style="background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 12px; border-radius: var(--radius-sm); font-size: 13px; outline: none;">
              <option value="auto" ${state.modePreference === 'auto' ? 'selected' : ''}>Auto (Recommended)</option>
              <option value="always-chill" ${state.modePreference === 'always-chill' ? 'selected' : ''}>Always Chill</option>
              <option value="always-hacker" ${state.modePreference === 'always-hacker' ? 'selected' : ''}>Always Hacker</option>
            </select>
          </div>

          <div style="display: flex; align-items: center; justify-content: space-between; padding: 12px 0;">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Manual Override</div>
              <div style="font-size: 12px; color: var(--ink-soft);">Switch active mode immediately</div>
            </div>
            <div style="display: flex; gap: 8px;">
              <button id="btn-force-chill" class="btn-review" style="padding: 6px 12px; font-size: 12px;">Chill</button>
              <button id="btn-force-hacker" class="btn-review" style="padding: 6px 12px; font-size: 12px;">Hacker</button>
            </div>
          </div>
        </div>

        <!-- Zero-Trust Safety Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin-bottom: 14px;">Zero-Trust Safety Engine</h2>
          
          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0;">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Safety Level</div>
              <div style="font-size: 12px; color: var(--ink-soft);">Strict: All plans need approval; YELLOW tasks always held; RED blocked</div>
            </div>
            <select id="select-safety" style="background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 12px; border-radius: var(--radius-sm); font-size: 13px; outline: none;">
              <option value="strict" selected>Strict (Default)</option>
              <option value="balanced">Balanced</option>
              <option value="relaxed">Relaxed</option>
            </select>
          </div>
        </div>

        <!-- AI Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;">
            <div>
              <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin: 0 0 4px 0;">AI</h2>
              <div style="font-size: 12px; color: var(--ink-soft);">NVIDIA API key used for chat.</div>
            </div>
            <div id="ai-key-status-badge"></div>
          </div>

          <div style="display: flex; flex-direction: column; gap: 14px; margin-top: 14px;">
            <div style="padding: 6px 0; border-bottom: 1px solid var(--border-subtle);">
              <div style="font-weight: 600; font-size: 13px; color: var(--ink); margin-bottom: 6px;">API Key</div>
              <div style="display: flex; gap: 8px;">
                <input id="input-ai-key" type="password" placeholder="nvapi-..." style="flex: 1; background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 8px 12px; border-radius: var(--radius-sm); font-size: 13px; font-family: var(--font-mono); outline: none;" />
                <button id="btn-toggle-key-vis" class="btn-review" style="padding: 6px 10px; font-size: 12px;" title="Show API key" aria-label="Show API key">${icon('eye', 18)}</button>
              </div>
            </div>

            <div style="display: flex; align-items: center; justify-content: space-between; padding: 6px 0;">
              <div style="font-weight: 600; font-size: 13px; color: var(--ink);">Model</div>
              <code class="font-mono" style="font-size: 12px; color: var(--ink);">nvidia/nemotron-3.5-bolt-e4b</code>
            </div>

            <div style="display: flex; align-items: center; justify-content: space-between; padding-top: 10px; border-top: 1px dashed var(--border-subtle);">
              <div id="ai-save-msg" style="font-size: 12px; font-weight: 500; max-width: 340px; word-break: break-word;"></div>
              <div style="display: flex; gap: 8px;">
                <button id="btn-test-ai-key" class="btn-review" style="padding: 6px 14px; font-size: 12px;">Test Connection</button>
                <button id="btn-clear-ai-key" class="btn-review" style="padding: 6px 14px; font-size: 12px;">Clear Key</button>
                <button id="btn-save-ai-key" class="btn-approve" style="padding: 6px 16px; font-size: 12px;">Save</button>
              </div>
            </div>
          </div>
        </div>

        <!-- System Doctor & Toolchain Scan Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 14px;">
            <h2 class="font-pixel" style="font-size: 17px; color: var(--ink);">System Doctor & Toolchain Scan</h2>
            <button id="btn-scan-doctor" class="btn-review" style="padding: 5px 12px; font-size: 12px;">Run Scan</button>
          </div>
          <div id="doctor-profile-output" style="font-size: 12px; color: var(--ink-soft); line-height: 1.6;">
            Click <strong>Run Scan</strong> to inspect host OS, GPU, and detected development tools via <code class="font-mono">SystemScanner</code>.
          </div>
        </div>

        <!-- Sound Engine Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin-bottom: 14px;">Cozy 8-Bit Audio</h2>
          
          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0;">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Sound Effects</div>
              <div style="font-size: 12px; color: var(--ink-soft);">Blips, sweeps, dings, and victory fanfare</div>
            </div>
            <input type="checkbox" id="check-sound" ${sounds.enabled ? 'checked' : ''} style="transform: scale(1.3); cursor: pointer;" />
          </div>

          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0; border-top: 1px solid var(--border-subtle); margin-top: 8px;">
            <div style="font-size: 13px; color: var(--ink-soft);">Volume</div>
            <input type="range" id="range-vol" min="0" max="1" step="0.05" value="${sounds.volume}" style="width: 140px; cursor: pointer;" />
          </div>
        </div>
      </div>
    </div>
  `;

  // Attach listeners
  container.querySelector('#select-mode').onchange = (e) => {
    const val = e.target.value;
    store.setState({ modePreference: val });
    if (val === 'always-chill') store.setMode('chill');
    if (val === 'always-hacker') store.setMode('hacker');
  };

  container.querySelector('#btn-force-chill').onclick = () => store.setMode('chill');
  container.querySelector('#btn-force-hacker').onclick = () => store.setMode('hacker');

  container.querySelector('#select-safety').onchange = (e) => {
    store.setState(s => ({ ...s, settings: { ...s.settings, safetyLevel: e.target.value } }));
    sounds.play('ui.click');
  };

  container.querySelector('#check-sound').onchange = (e) => {
    sounds.setMuted(!e.target.checked);
    if (e.target.checked) sounds.play('task.done');
  };

  container.querySelector('#range-vol').oninput = (e) => {
    sounds.setVolume(parseFloat(e.target.value));
  };

  const btnScanDoctor = container.querySelector('#btn-scan-doctor');
  if (btnScanDoctor) {
    btnScanDoctor.onclick = async () => {
      sounds.play('ui.click');
      const out = container.querySelector('#doctor-profile-output');
      if (out) out.innerHTML = `<span style="color: var(--accent);">⟳ Scanning host machine via Rust SystemScanner...</span>`;
      try {
        const doc = await apiClient.getDoctor();
        sounds.play('verdict.green');
        const toolsHtml = (doc.installed_tools || [])
          .map(t => `<li style="margin-bottom: 2px;"><strong>${t.binary}</strong>: <code class="font-mono">${t.version_output || 'detected'}</code> (<span class="font-mono">${t.path}</span>)</li>`)
          .join('');
        if (out) {
          out.innerHTML = `
            <div style="background: rgba(0,0,0,0.04); border-radius: var(--radius-md); padding: 12px; margin-top: 6px;">
              <div><strong>OS:</strong> ${doc.os_name} (${doc.os_version}) · <strong>Arch:</strong> ${doc.arch}</div>
              <div><strong>GPU:</strong> ${doc.gpu_detected || 'Standard'}</div>
              <div style="margin-top: 8px; font-weight: 600;">Detected Tools (${(doc.installed_tools || []).length}):</div>
              <ul style="padding-left: 18px; margin: 4px 0 0 0;">${toolsHtml}</ul>
            </div>
          `;
        }
      } catch (err) {
        sounds.play('error');
        if (out) out.innerHTML = `<span style="color: var(--v-red);">Scan error: ${err.message}</span>`;
      }
    };
  }

  // AI Configuration (single fixed NVIDIA model; only the key is configurable)
  const badgeEl = container.querySelector('#ai-key-status-badge');
  const keyInput = container.querySelector('#input-ai-key');
  const btnToggleVis = container.querySelector('#btn-toggle-key-vis');
  const btnSaveKey = container.querySelector('#btn-save-ai-key');
  const btnClearKey = container.querySelector('#btn-clear-ai-key');
  const btnTestKey = container.querySelector('#btn-test-ai-key');
  const msgEl = container.querySelector('#ai-save-msg');

  const escapeHtml = (s) => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const setMsg = (text, color) => {
    if (msgEl) msgEl.innerHTML = `<span style="color: ${color};">${escapeHtml(text)}</span>`;
  };

  const updateAiBadge = () => {
    const key = localStorage.getItem('setuppal_ai_api_key') || '';
    if (key.trim()) {
      const masked = key.length > 10 ? `${key.slice(0, 6)}...${key.slice(-4)}` : '••••••';
      badgeEl.innerHTML = `<span style="background: rgba(34, 197, 94, 0.15); color: #16a34a; font-size: 11px; font-weight: 600; padding: 4px 10px; border-radius: var(--radius-pill); border: 1px solid rgba(34, 197, 94, 0.3);">● Key saved (${masked})</span>`;
    } else {
      badgeEl.innerHTML = `<span style="background: rgba(239, 68, 68, 0.1); color: var(--v-red); font-size: 11px; font-weight: 600; padding: 4px 10px; border-radius: var(--radius-pill); border: 1px solid rgba(239, 68, 68, 0.25);">○ No key</span>`;
    }
  };

  // Drop settings left over from the old provider/model/endpoint selectors
  ['setuppal_ai_provider', 'setuppal_ai_model', 'setuppal_ai_endpoint'].forEach(k => localStorage.removeItem(k));

  if (keyInput) keyInput.value = localStorage.getItem('setuppal_ai_api_key') || '';
  updateAiBadge();

  if (btnToggleVis && keyInput) {
    btnToggleVis.onclick = () => {
      keyInput.type = keyInput.type === 'password' ? 'text' : 'password';
      const isHidden = keyInput.type === 'password';
      btnToggleVis.innerHTML = icon(isHidden ? 'eye' : 'eyeOff', 18);
      btnToggleVis.title = isHidden ? 'Show API key' : 'Hide API key';
      btnToggleVis.setAttribute('aria-label', btnToggleVis.title);
    };
  }

  if (btnSaveKey) {
    btnSaveKey.onclick = () => {
      const keyVal = (keyInput?.value || '').trim();
      if (keyVal) localStorage.setItem('setuppal_ai_api_key', keyVal);
      else localStorage.removeItem('setuppal_ai_api_key');
      sounds.play('task.done');
      updateAiBadge();
      setMsg('Saved.', '#16a34a');
      setTimeout(() => { if (msgEl) msgEl.innerHTML = ''; }, 3000);
    };
  }

  if (btnTestKey) {
    btnTestKey.onclick = async () => {
      const keyVal = (keyInput?.value || '').trim() || localStorage.getItem('setuppal_ai_api_key') || '';
      if (!keyVal) {
        setMsg('Enter an API key first.', 'var(--v-red)');
        return;
      }
      setMsg('Testing...', 'var(--accent)');
      sounds.play('ui.click');
      try {
        const res = await apiClient.validateKey({ apiKey: keyVal });
        sounds.play(res.valid ? 'verdict.green' : 'verdict.red');
        setMsg(res.message, res.valid ? '#16a34a' : 'var(--v-red)');
      } catch (err) {
        sounds.play('error');
        setMsg(`Error: ${err.message}`, 'var(--v-red)');
      }
    };
  }

  if (btnClearKey) {
    btnClearKey.onclick = () => {
      localStorage.removeItem('setuppal_ai_api_key');
      if (keyInput) keyInput.value = '';
      sounds.play('ui.click');
      updateAiBadge();
      setMsg('Key cleared.', 'var(--ink-soft)');
      setTimeout(() => { if (msgEl) msgEl.innerHTML = ''; }, 3000);
    };
  }
}