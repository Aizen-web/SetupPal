/**
 * SetupPal: Settings View (ui.md Section 9.5)
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';

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

        <!-- Mascot Pet Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin-bottom: 14px;">Active Kawaii Pet Companion</h2>
          
          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0;">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Companion Pet</div>
              <div style="font-size: 12px; color: var(--ink-soft);">Each pet features 20 unique animations and distinct personalities</div>
            </div>
            <select id="select-active-pet" style="background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 12px; border-radius: var(--radius-sm); font-size: 13px; outline: none;">
              <option value="ghost" ${state.activePet === 'ghost' ? 'selected' : ''}>👻 Pixel the Ghost</option>
              <option value="cat" ${state.activePet === 'cat' ? 'selected' : ''}>🐱 Byte the Cat</option>
              <option value="bunny" ${state.activePet === 'bunny' ? 'selected' : ''}>🐰 Mochi the Bunny</option>
            </select>
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

  container.querySelector('#select-active-pet').onchange = (e) => {
    store.setActivePet(e.target.value);
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
}
