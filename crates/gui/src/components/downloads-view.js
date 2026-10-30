/**
 * SetupPal: Downloads View
 * Lists artifacts the orchestrator actually downloaded (from the backend transaction log).
 */

import { store } from '../state/store.js';
import { esc } from './html.js';
import { icon } from './icon.js';

export function renderDownloadsView(container) {
  const downloads = store.getState().downloads || [];

  const body = downloads.length === 0
    ? `<tr><td colspan="4" class="empty-note">No downloads yet. Verified artifacts will be listed here after an install runs.</td></tr>`
    : downloads.map(d => {
        const sha = String(d.sha256 || '');
        return `
        <tr>
          <td class="cell-strong">${icon('package', 16)} ${esc(d.filename)}</td>
          <td>${esc(d.source)}</td>
          <td><span class="font-mono cell-muted" title="${esc(sha)}">${sha ? `${esc(sha.slice(0, 16))}${sha.length > 16 ? '…' : ''}` : 'none'}</span></td>
          <td class="cell-muted">${esc(d.date)}${d.rolledBack ? ' · rewound' : ''}</td>
        </tr>`;
      }).join('');

  container.innerHTML = `
    <div class="main-content page">
      <div class="page-inner page-inner-wide">
        <header class="page-header">
          <h1 class="page-title">Downloads</h1>
          <p class="page-sub">Artifacts fetched and hash-verified by the orchestrator.</p>
        </header>

        <div class="card card-flush">
          <table class="data-table">
            <thead>
              <tr>
                <th scope="col">File</th>
                <th scope="col">Source</th>
                <th scope="col">Expected SHA-256</th>
                <th scope="col">When</th>
              </tr>
            </thead>
            <tbody>${body}</tbody>
          </table>
        </div>
      </div>
    </div>
  `;
}
