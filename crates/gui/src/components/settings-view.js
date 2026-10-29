/**
 * SetupPal: Settings View (ui.md Section 9.5)
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { apiClient } from '../api/client.js';
import { getPrefs, setPrefs, resetPrefs, REPLY_LENGTHS } from '../state/prefs.js';

export function renderSettingsView(container) {
  const state = store.getState();
  const settings = state.settings;
  const prefs = getPrefs();

  container.innerHTML = `
    <div class="main-content" style="padding: 24px 32px; overflow-y: auto;">
      <div style="margin-bottom: 24px;">
        <h1 class="font-pixel" style="font-size: 24px; color: var(--ink);">Settings</h1>
        <div style="font-size: 13px; color: var(--ink-soft); margin-top: 4px;">
          Appearance, safety, chat memory, and audio.
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

          <div style="display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 10px 0 2px; border-top: 1px solid var(--border-subtle);">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Ambient background</div>
              <div style="font-size: 12px; color: var(--ink-soft);">A quiet theme-matched glow behind the workspace</div>
            </div>
            <select id="select-background" style="background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 12px; border-radius: var(--radius-sm); font-size: 13px;">
              <option value="off" ${settings.backgroundIntensity === 'off' ? 'selected' : ''}>Off</option>
              <option value="subtle" ${!settings.backgroundIntensity || settings.backgroundIntensity === 'subtle' ? 'selected' : ''}>Subtle</option>
              <option value="lively" ${settings.backgroundIntensity === 'lively' ? 'selected' : ''}>Lively</option>
            </select>
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

        <!-- Chat & Memory Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin-bottom: 4px;">Chat & Memory</h2>
          <div style="font-size: 12px; color: var(--ink-soft); margin-bottom: 10px;">Older messages are folded into short notes, so Pixel remembers what you said without sending the whole chat every time.</div>

          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0; border-bottom: 1px solid var(--border-subtle);">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Recent messages kept in full</div>
              <div style="font-size: 12px; color: var(--ink-soft);">More means better recall but more tokens</div>
            </div>
            <select id="select-memory" style="background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 12px; border-radius: var(--radius-sm); font-size: 13px; outline: none;">
              ${[4, 6, 10, 16].map(n => `<option value="${n}" ${prefs.memoryTurns === n ? 'selected' : ''}>${n}${n === 6 ? ' (Default)' : ''}</option>`).join('')}
            </select>
          </div>

          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0; border-bottom: 1px solid var(--border-subtle);">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Reply length</div>
              <div style="font-size: 12px; color: var(--ink-soft);">Upper limit on how long answers can get</div>
            </div>
            <select id="select-reply-length" style="background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 12px; border-radius: var(--radius-sm); font-size: 13px; outline: none;">
              ${Object.entries(REPLY_LENGTHS).map(([k, v]) => `<option value="${k}" ${prefs.replyLength === k ? 'selected' : ''}>${v.label} (${v.tokens})</option>`).join('')}
            </select>
          </div>

          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0; border-bottom: 1px solid var(--border-subtle);">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Share PC facts with the AI</div>
              <div style="font-size: 12px; color: var(--ink-soft);">OS, GPU, free disk, and installed dev tools. Needed for good recommendations</div>
            </div>
            <input type="checkbox" id="check-pc-facts" ${prefs.includePcFacts ? 'checked' : ''} style="transform: scale(1.3); cursor: pointer;" />
          </div>

          <div style="display: flex; align-items: center; justify-content: space-between; padding: 12px 0 2px;">
            <div style="font-size: 12px; color: var(--ink-soft);">Model: <code class="font-mono">nvidia/nemotron-3.5-bolt-e4b</code> · API key lives on the Accounts page</div>
            <div style="display: flex; gap: 8px;">
              <button id="btn-clear-chats" class="btn-review" style="padding: 6px 12px; font-size: 12px; color: var(--v-red);">Clear all chats</button>
              <button id="btn-reset-prefs" class="btn-review" style="padding: 6px 12px; font-size: 12px;">Reset</button>
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

        <!-- Mascot Lab Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <div style="display: flex; align-items: center; justify-content: space-between; gap: 12px;">
            <div>
              <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin-bottom: 4px;">Mascot Lab</h2>
              <div style="font-size: 12px; color: var(--ink-soft);">Preview Pixel's animations and pet states.</div>
            </div>
            <button id="btn-open-mascot-lab" class="btn-review" style="padding: 6px 12px; font-size: 12px; flex: none;">Open lab</button>
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

  container.querySelector('#select-background').onchange = (event) => {
    store.setState(s => ({
      ...s,
      settings: { ...s.settings, backgroundIntensity: event.target.value }
    }));
    sounds.play('ui.click');
  };

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

  // Chat & memory preferences
  container.querySelector('#select-memory').onchange = (e) => {
    setPrefs({ memoryTurns: Number(e.target.value) });
    sounds.play('ui.click');
  };
  container.querySelector('#select-reply-length').onchange = (e) => {
    setPrefs({ replyLength: e.target.value });
    sounds.play('ui.click');
  };
  container.querySelector('#check-pc-facts').onchange = (e) => {
    setPrefs({ includePcFacts: e.target.checked });
    sounds.play('ui.click');
  };
  container.querySelector('#btn-reset-prefs').onclick = () => {
    resetPrefs();
    sounds.play('ui.click');
    renderSettingsView(container);
  };
  container.querySelector('#btn-clear-chats').onclick = () => {
    if (!confirm('Delete all saved chats? This cannot be undone.')) return;
    ['setuppal_local_conversations_v2', 'setuppal_active_conv_id_v2'].forEach(k => localStorage.removeItem(k));
    sounds.play('ui.click');
    location.reload();
  };

  container.querySelector('#btn-open-mascot-lab').onclick = () => {
    sounds.play('ui.click');
    store.toggleAnimationLab(true);
  };
}
