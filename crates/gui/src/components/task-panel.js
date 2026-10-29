/**
 * SetupPal: Task & Progress Panel (Right Sidebar)
 * Shown only when a plan exists. Pixel stays the sole pet in the empty chat.
 */

import { store } from '../state/store.js';
import { createSleepingMascotOnBooksSvg } from '../mascot/mascot-art.js';
import { icon } from './icon.js';

export function renderTaskPanel(container) {
  const state = store.getState();
  const isHacker = state.mode === 'hacker';
  const plan = state.activePlan;
  const tasks = plan?.tasks || [];

  if (tasks.length === 0) {
    container.innerHTML = '';
    container.classList.add('task-panel-collapsed');
    return;
  }

  container.classList.remove('task-panel-collapsed');

  const approvedTasks = tasks.filter(t => t.status !== 'held' && t.status !== 'blocked');
  const doneCount = approvedTasks.filter(t => t.status === 'done').length;
  const totalApproved = approvedTasks.length;
  const progressPct = totalApproved > 0 ? Math.round((doneCount / totalApproved) * 100) : 0;
  const heldCount = tasks.filter(t => t.status === 'held').length;
  const isRunning = tasks.some(t => t.status === 'running');

  container.innerHTML = `
    <div class="task-panel">
      <div class="task-panel-header">
        <div class="task-panel-title-row">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"></path>
            <rect x="8" y="2" width="8" height="4" rx="1" ry="1"></rect>
          </svg>
          <span class="task-panel-title">${isHacker ? 'Progress' : 'Tasks'}</span>
        </div>
        <div class="task-panel-subhead">${doneCount} of ${totalApproved} done</div>

        ${totalApproved > 0 ? `
          <div class="progress-track" style="margin-top: 10px;">
            <div class="progress-fill" style="width: ${progressPct}%;"></div>
          </div>
          ${heldCount > 0 ? `
            <div class="progress-subline">${heldCount} held</div>
          ` : ''}
        ` : ''}
      </div>

      <div class="task-panel-divider"></div>

      <div class="task-list">
        ${tasks.map(t => {
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
                  <div class="task-item-meta">
                    <span>${t.sourceDomain ? t.sourceDomain : t.source}</span>
                    ${t.verdict === 'YELLOW' ? '<span class="task-meta-caution">· Caution</span>' : ''}
                    ${t.verdict === 'RED' ? '<span class="task-meta-blocked">· Blocked</span>' : ''}
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

      ${isRunning && !isHacker ? `
        <div class="task-panel-bottom-art">
          ${createSleepingMascotOnBooksSvg(120, 100)}
        </div>
      ` : ''}

      ${isHacker ? `
        <div class="task-panel-bottom-art">
          <div class="task-panel-zt-tag">[ZERO-TRUST ACTIVE]</div>
        </div>
      ` : ''}
    </div>
  `;
}
