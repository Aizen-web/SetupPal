/**
 * SetupPal: Task & Progress Panel (Right Sidebar)
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';

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
      <div class="task-panel-header">
        <div class="task-panel-title">${isHacker ? 'Progress' : 'TASKS'}</div>
        <div class="task-panel-counter">${totalApproved > 0 ? `${doneCount} of ${totalApproved} done` : 'No active tasks'}</div>
        
        <div class="progress-track">
          <div class="progress-fill" style="width: ${progressPct}%;"></div>
        </div>
        
        ${heldCount > 0 ? `
          <div class="progress-subline">${heldCount} held · 0 blocked</div>
        ` : ''}
      </div>

      <div class="task-list">
        ${tasks.length === 0 ? `
          <div style="padding: 32px 16px; text-align: center; color: var(--ink-soft); font-size: 12.5px; line-height: 1.5;">
            No active plan yet.<br/>Type in the chat to define a goal and formulate verified steps.
          </div>
        ` : tasks.map(t => {
          let iconHtml = '';
          let chipClass = '';

          if (t.status === 'done') {
            iconHtml = `<div class="task-status-icon done">✓</div>`;
            chipClass = 'badge-green';
          } else if (t.status === 'running') {
            iconHtml = `<div class="task-status-icon running">⟳</div>`;
            chipClass = 'badge-green';
          } else if (t.status === 'held') {
            iconHtml = `<div class="task-status-icon held">!</div>`;
            chipClass = 'badge-yellow';
          } else if (t.status === 'blocked') {
            iconHtml = `<div class="task-status-icon" style="border-color: var(--v-red); color: var(--v-red); background: rgba(196,73,58,0.15);">✕</div>`;
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
    </div>
  `;
}
