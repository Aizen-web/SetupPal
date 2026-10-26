/**
 * SetupPal: Task & Progress Panel (Right Sidebar)
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';

export function renderTaskPanel(container) {
  const state = store.getState();
  const isHacker = state.mode === 'hacker';
  const plan = state.activePlan;

  // Calculate done tasks count out of approved tasks
  const approvedTasks = plan.tasks.filter(t => t.status !== 'held' && t.status !== 'blocked');
  const doneCount = approvedTasks.filter(t => t.status === 'done').length;
  const totalApproved = approvedTasks.length;
  const progressPct = totalApproved > 0 ? Math.round((doneCount / totalApproved) * 100) : 0;
  const heldCount = plan.tasks.filter(t => t.status === 'held').length;

  container.innerHTML = `
    <div class="task-panel">
      <div class="task-panel-header">
        <div class="task-panel-title">${isHacker ? 'Progress' : 'TASKS'}</div>
        <div class="task-panel-counter">${doneCount} of ${totalApproved} done</div>
        
        <div class="progress-track">
          <div class="progress-fill" style="width: ${progressPct}%;"></div>
        </div>
        
        ${heldCount > 0 ? `
          <div class="progress-subline">${heldCount} held · 0 blocked</div>
        ` : ''}
      </div>

      <div class="task-list">
        ${plan.tasks.map(t => {
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
          } else {
            iconHtml = `<div class="task-status-icon waiting">${t.num}</div>`;
            chipClass = '';
          }

          return `
            <div class="task-item ${t.status === 'running' ? 'active' : ''}">
              <div class="task-item-left">
                ${iconHtml}
                <div class="task-item-title">${t.title.replace('Install ', '').replace('Add ', '')}</div>
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
