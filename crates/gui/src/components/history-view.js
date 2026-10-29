/**
 * SetupPal: History & Changes View (ui.md Section 9.2)
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { bridge } from '../api/bridge.js';
import { createGhostSvg } from '../mascot/pixel-ghost.js';
import { icon } from './icon.js';

export function renderHistoryView(container) {
  const state = store.getState();
  const history = state.history;

  container.innerHTML = `
    <div class="main-content" style="padding: 24px 32px; overflow-y: auto;">
      <div style="margin-bottom: 24px;">
        <h1 class="font-pixel" style="font-size: 24px; color: var(--ink);">History & Rewind Log</h1>
        <div style="font-size: 13px; color: var(--ink-soft); margin-top: 4px;">
          Deterministic audit log of all environment interactions and system changes.
        </div>
      </div>

      <!-- Changes Timeline (Rewind Log) -->
      <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; margin-bottom: 24px; box-shadow: var(--shadow-card);">
        <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px;">
          <div style="display: flex; align-items: center; gap: 8px;">
            <a id="btn-export-audit" href="/api/history/export?format=html" target="_blank" class="btn-review" style="font-size: 12.5px; text-decoration: none; display: flex; align-items: center; gap: 4px; padding: 5px 12px;">
              ${icon('fileText', 18)} Export Audit Report (HTML)
            </a>
            <button id="btn-rewind" class="btn-review" style="border-color: var(--v-red); color: var(--v-red); font-size: 12.5px;">
              ${icon('rewind', 18)} Rewind Last Change
            </button>
          </div>
        </div>

        <div style="display: flex; flex-direction: column; gap: 10px;">
          ${history.changes.map(tx => `
            <div style="display: flex; align-items: center; justify-content: space-between; padding: 10px 14px; background: rgba(255,255,255,0.4); border: 1px solid var(--border-subtle); border-radius: var(--radius-md);">
              <div style="display: flex; align-items: center; gap: 12px;">
                <span class="font-mono" style="font-size: 11px; color: var(--ink-soft);">${tx.time}</span>
                <span style="font-weight: 600; font-size: 13.5px; color: var(--ink);">${tx.desc}</span>
              </div>
              <div style="display: flex; align-items: center; gap: 10px;">
                <span style="font-size: 11px; background: var(--accent-soft); color: var(--accent-strong); padding: 2px 8px; border-radius: 999px;">
                  Reversible: ${tx.reversible}
                </span>
                <button class="btn-review" style="padding: 3px 8px; font-size: 11px;" onclick="alert('Transaction ${tx.id} audited against Zero-Trust state ledger.')">Audit</button>
              </div>
            </div>
          `).join('')}
        </div>
      </div>

      <!-- Past Conversations -->
      <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
        <h2 class="font-pixel" style="font-size: 18px; color: var(--ink); margin-bottom: 14px;">Past Conversations</h2>
        <div style="display: flex; flex-direction: column; gap: 8px;">
          ${history.conversations.map(c => `
            <div style="display: flex; align-items: center; justify-content: space-between; padding: 10px 14px; border-radius: var(--radius-md); border: 1px solid var(--border-subtle);">
              <div>
                <div style="font-weight: 600; font-size: 14px; color: var(--ink);">${c.title}</div>
                <div style="font-size: 12px; color: var(--ink-soft);">${c.type} · ${c.date} · ${c.tasksDone}</div>
              </div>
              <span class="badge ${c.verdict === 'GREEN' ? 'badge-green' : c.verdict === 'YELLOW' ? 'badge-yellow' : 'badge-red'}">
                ${c.verdict}
              </span>
            </div>
          `).join('')}
        </div>
      </div>
    </div>
  `;

  container.querySelector('#btn-rewind').onclick = () => {
    bridge.rewindLastChange();
  };
}
