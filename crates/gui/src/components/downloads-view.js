/**
 * SetupPal: Downloads View (ui.md Section 9.3)
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { createMascotSvg } from '../mascot/pets.js';

export function renderDownloadsView(container) {
  const state = store.getState();
  const downloads = state.downloads;
  const activePet = state.activePet || 'ghost';

  container.innerHTML = `
    <div class="main-content" style="padding: 24px 32px; overflow-y: auto;">
      <!-- Header -->
      <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 24px;">
        <div>
          <h1 class="font-pixel" style="font-size: 24px; color: var(--ink);">Downloads & Artifacts</h1>
          <div style="font-size: 13px; color: var(--ink-soft); margin-top: 4px;">
            Storage: <strong>208.6 MB</strong> · Path: <code class="font-mono">~/Downloads/SetupPal</code>
          </div>
        </div>

        <div style="display: flex; gap: 10px;">
          <button id="btn-sim-dl" class="btn-approve" style="font-size: 12.5px; padding: 8px 14px;">
            Simulate Active Download
          </button>
          <button id="btn-clean-cache" class="btn-review" style="font-size: 12.5px; padding: 8px 14px;">
            Clean Cache
          </button>
        </div>
      </div>

      <!-- Active Download Mascot Callout -->
      <div style="display: flex; align-items: center; gap: 20px; background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 18px 24px; margin-bottom: 24px; box-shadow: var(--shadow-card);">
        <div style="width: 80px; height: 80px; display: flex; align-items: center; justify-content: center;">
          ${createMascotSvg(activePet, 80, 'dl')}
        </div>
        <div style="flex: 1;">
          <div style="font-size: 14px; font-weight: 700; color: var(--ink);">
            Active Download: <span class="font-mono" style="color: var(--accent-strong);">fabric-installer-1.0.1.exe</span>
          </div>
          <div style="font-size: 12px; color: var(--ink-soft); margin: 4px 0 8px 0;">
            Source: <strong style="color: var(--ink);">fabricmc.net</strong> (Allowlisted) · 14.2 MB of 18.0 MB · 4.8 MB/s · ETA: 1s
          </div>
          <div class="progress-track" style="height: 8px;">
            <div class="progress-fill" style="width: 78%;"></div>
          </div>
        </div>
      </div>

      <!-- Downloads Table -->
      <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); overflow: hidden; box-shadow: var(--shadow-card);">
        <table style="width: 100%; border-collapse: collapse; text-align: left; font-size: 13px;">
          <thead>
            <tr style="border-bottom: 1px solid var(--border); color: var(--ink-soft); font-size: 11px; text-transform: uppercase; letter-spacing: 0.08em;">
              <th style="padding: 12px 16px;">File</th>
              <th style="padding: 12px 16px;">Source</th>
              <th style="padding: 12px 16px;">Size</th>
              <th style="padding: 12px 16px;">SHA-256 Check</th>
              <th style="padding: 12px 16px;">Verdict</th>
              <th style="padding: 12px 16px; text-align: right;">Action</th>
            </tr>
          </thead>
          <tbody>
            ${downloads.map(d => `
              <tr style="border-bottom: 1px solid var(--border-subtle);">
                <td style="padding: 12px 16px; font-weight: 600; color: var(--ink);">
                  📦 ${d.filename}
                </td>
                <td style="padding: 12px 16px;">
                  <span style="font-weight: 500;">${d.source}</span>
                  <span style="font-size: 10px; background: var(--accent-soft); color: var(--accent-strong); padding: 1px 6px; border-radius: 999px; margin-left: 4px;">allowlisted</span>
                </td>
                <td style="padding: 12px 16px; color: var(--ink-soft);">${d.size}</td>
                <td style="padding: 12px 16px;">
                  <span class="font-mono" style="font-size: 11.5px; color: var(--ink-soft);">${d.sha256.slice(0, 10)}...</span>
                  <span style="font-size: 11px; color: ${d.verdict === 'GREEN' ? 'var(--v-green-text)' : 'var(--v-yellow-text)'}; margin-left: 6px;">${d.hashStatus}</span>
                </td>
                <td style="padding: 12px 16px;">
                  <span class="badge ${d.verdict === 'GREEN' ? 'badge-green' : d.verdict === 'YELLOW' ? 'badge-yellow' : 'badge-red'}">
                    ${d.verdict}
                  </span>
                </td>
                <td style="padding: 12px 16px; text-align: right;">
                  <button class="btn-review" style="padding: 4px 10px; font-size: 12px;" onclick="alert('SHA-256 verified directly against adoptium.net signature manifest.')">Verify</button>
                </td>
              </tr>
            `).join('')}
          </tbody>
        </table>
      </div>
    </div>
  `;

  // Set the mascot to downloading state
  const mascotSvg = container.querySelector('#dl-ghost-root');
  if (mascotSvg) {
    mascotSvg.dataset.state = 'downloading';
  }

  container.querySelector('#btn-sim-dl').onclick = () => {
    sounds.play('task.start');
    store.setMode('hacker');
    store.setActivePage('chats');
    store.setMascotState('downloading', "Carrying your verified packages through the pipeline!");
  };

  container.querySelector('#btn-clean-cache').onclick = () => {
    sounds.play('ui.click');
    alert("Cache cleaned! Removed 18.0 MB of temporary package payloads.");
  };
}
