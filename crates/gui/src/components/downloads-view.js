/**
 * SetupPal: Downloads View
 * Lists artifacts the orchestrator actually downloaded (from the backend transaction log).
 */

import { store } from '../state/store.js';
import { icon } from './icon.js';

function esc(value) {
  return String(value ?? '').replace(/[&<>"']/g, ch => (
    { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]
  ));
}

export function renderDownloadsView(container) {
  const downloads = store.getState().downloads || [];

  const body = downloads.length === 0
    ? `<tr><td colspan="4" style="padding: 20px 16px; color: var(--ink-soft);">No downloads yet. Verified artifacts will be listed here after an install runs.</td></tr>`
    : downloads.map(d => `
        <tr style="border-bottom: 1px solid var(--border-subtle);">
          <td style="padding: 12px 16px; font-weight: 600; color: var(--ink);">${icon('package', 16)} ${esc(d.filename)}</td>
          <td style="padding: 12px 16px;">${esc(d.source)}</td>
          <td style="padding: 12px 16px;"><span class="font-mono" style="font-size: 11.5px; color: var(--ink-soft);" title="${esc(d.sha256)}">${esc(d.sha256.slice(0, 16))}${d.sha256.length > 16 ? '...' : ''}</span></td>
          <td style="padding: 12px 16px; color: var(--ink-soft);">${esc(d.date)}${d.rolledBack ? ' · rewound' : ''}</td>
        </tr>`).join('');

  container.innerHTML = `
    <div class="main-content" style="padding: 24px 32px; overflow-y: auto;">
      <div style="margin-bottom: 24px;">
        <h1 class="font-pixel" style="font-size: 24px; color: var(--ink);">Downloads & Artifacts</h1>
        <div style="font-size: 13px; color: var(--ink-soft); margin-top: 4px;">
          Artifacts fetched and hash-verified by the orchestrator.
        </div>
      </div>

      <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); overflow: hidden; box-shadow: var(--shadow-card);">
        <table style="width: 100%; border-collapse: collapse; text-align: left; font-size: 13px;">
          <thead>
            <tr style="border-bottom: 1px solid var(--border); color: var(--ink-soft); font-size: 11px; text-transform: uppercase; letter-spacing: 0.08em;">
              <th style="padding: 12px 16px;">File</th>
              <th style="padding: 12px 16px;">Source</th>
              <th style="padding: 12px 16px;">Expected SHA-256</th>
              <th style="padding: 12px 16px;">When</th>
            </tr>
          </thead>
          <tbody>${body}</tbody>
        </table>
      </div>
    </div>
  `;
}
