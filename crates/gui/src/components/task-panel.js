/**
 * SetupPal: Task & Progress Panel (Right Sidebar)
 * Matching the exact artwork and copy of the screenshot,
 * fully styled for both Chill and Hacker modes.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { createClipboardTaskArtSvg, createSleepingMascotOnBooksSvg } from '../mascot/mascot-art.js';
import { icon } from './icon.js';

export function renderTaskPanel(container) {
  const state = store.getState();
  const isHacker = state.mode === 'hacker';
  const plan = state.activePlan;
  const tasks = plan?.tasks || [];

  // Calculate done tasks count out of approved tasks
  const approvedTasks = tasks.filter(t => t.status !== 'held' && t.status !== 'blocked');
  const doneCount = approvedTasks.filter(t => t.status === 'done').length;
  const totalApproved = approvedTasks.length;
  const progressPct = totalApproved > 0 ? Math.round((doneCount / totalApproved) * 100) : 0;
  const heldCount = tasks.filter(t => t.status === 'held').length;

  container.innerHTML = `
    <div class="task-panel">
      <!-- Top Action Bar (Search & Settings icons) -->
      <div class="task-panel-top-actions">
        <button class="task-icon-btn" id="btn-task-search" title="Search setups & packages">
          ${icon('search')}
        </button>
        <button class="task-icon-btn" id="btn-task-settings" title="Settings">
          <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="3"></circle>
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06-.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
          </svg>
        </button>
      </div>

      <!-- Header Row: [Clipboard icon] Tasks -->
      <div class="task-panel-header">
        <div class="task-panel-title-row">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"></path>
            <rect x="8" y="2" width="8" height="4" rx="1" ry="1"></rect>
          </svg>
          <span class="task-panel-title">${isHacker ? 'Progress' : 'Tasks'}</span>
        </div>
        <div class="task-panel-subhead">${totalApproved > 0 ? `${doneCount} of ${totalApproved} done` : 'No active tasks'}</div>
        
        ${totalApproved > 0 ? `
          <div class="progress-track" style="margin-top: 10px;">
            <div class="progress-fill" style="width: ${progressPct}%;"></div>
          </div>
          ${heldCount > 0 ? `
            <div class="progress-subline">${heldCount} held · 0 blocked</div>
          ` : ''}
        ` : ''}
      </div>

      <div class="task-panel-divider"></div>

      <!-- Main Body: Empty State or Active Steps -->
      <div class="task-list">
        ${tasks.length === 0 ? `
          <!-- Screenshot Empty State: Clipboard Art + Copy -->
          <div class="task-empty-state">
            <div class="task-empty-art-wrap">
              ${createClipboardTaskArtSvg(92)}
            </div>
            <div class="task-empty-title">No active plan yet.</div>
            <div class="task-empty-desc">
              Type in the chat to define a goal and<br/>formulate verified steps.
            </div>
          </div>
        ` : tasks.map(t => {
          let iconHtml = '';
          let chipClass = '';

          if (t.status === 'done') {
            iconHtml = `<div class="task-status-icon done">${icon('check', 14)}</div>`;
            chipClass = 'badge-green';
          } else if (t.status === 'running') {
            iconHtml = `<div class="task-status-icon running">${icon('refresh', 14)}</div>`;
            chipClass = 'badge-green';
          } else if (t.status === 'held') {
            iconHtml = `<div class="task-status-icon held">${icon('alert', 14)}</div>`;
            chipClass = 'badge-yellow';
          } else if (t.status === 'blocked') {
            iconHtml = `<div class="task-status-icon blocked">${icon('close', 14)}</div>`;
            chipClass = 'badge-red';
          } else {
            iconHtml = `<div class="task-status-icon waiting">${t.num}</div>`;
            chipClass = '';
          }

          return `
            <div class="task-item ${t.status === 'running' ? 'active' : ''}">
              <div class="task-item-left">
                ${iconHtml}
                <div>
                  <div class="task-item-title">${t.title.replace('Install ', '').replace('Add ', '')}</div>
                  <div style="font-size: 10px; color: var(--ink-soft); display: flex; align-items: center; gap: 4px; margin-top: 1px;">
                    <span>${t.sourceDomain ? t.sourceDomain : t.source}</span>
                    ${t.verdict === 'YELLOW' ? '<span style="color: #b26a00;">· Caution</span>' : ''}
                    ${t.verdict === 'RED' ? '<span style="color: #c62828;">· Blocked</span>' : ''}
                  </div>
                </div>
              </div>
              <div class="status-chip-sm ${chipClass}">
                ${t.status.toUpperCase()}
              </div>
            </div>
          `;
        }).join('')}
      </div>

      <!-- Bottom Mascot Art: Sleeping Chubby Mascot on Books in Chill mode / Clean status in Hacker mode -->
      <div class="task-panel-bottom-art">
        ${!isHacker ? createSleepingMascotOnBooksSvg(155, 125) : `
          <div style="padding: 14px 10px; font-family: var(--font-mono); font-size: 11px; color: var(--accent); opacity: 0.7; text-align: center; letter-spacing: 0.05em;">
            <span>[ZERO-TRUST ACTIVE]</span>
          </div>
        `}
      </div>
    </div>
  `;

  container.querySelector('#btn-task-settings')?.remove();

  const btnSearch = container.querySelector('#btn-task-search');
  if (btnSearch) {
    btnSearch.onclick = () => {
      const chatInput = document.querySelector('#chat-input');
      if (chatInput) {
        chatInput.focus();
        chatInput.placeholder = "Search packages or describe an environment...";
      }
    };
  }
}
