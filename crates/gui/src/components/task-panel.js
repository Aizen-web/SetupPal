/**
 * SetupPal: Task & Progress Panel (right side)
 * Exists only while the chat has a plan. Collapses to a slim rail that can be reopened.
 */

import { store } from '../state/store.js';
import { esc } from './html.js';
import { icon } from './icon.js';
import { renderPlanCard, renderCompletedPlan, bindPlanCard, isPlanReviewable } from './plan-card.js';

// status -> [icon, badge class, label]
const STATUS = {
  done: ['check', 'badge-green', 'Done'],
  running: ['refresh', 'badge-green', 'Running'],
  held: ['alert', 'badge-yellow', 'Held'],
  blocked: ['shieldX', 'badge-red', 'Blocked'],
  failed: ['close', 'badge-red', 'Failed'],
  skipped: ['arrowRight', '', 'Skipped'],
  cancelled: ['close', '', 'Cancelled'],
};

export function renderTaskPanel(container) {
  const state = store.getState();
  const tasks = state.activePlan?.tasks || [];
  if (tasks.length === 0) {
    container.innerHTML = '';
    return;
  }

  const plan = state.activePlan;
  const reviewable = isPlanReviewable(plan);
  const count = (status) => tasks.filter(t => t.status === status).length;
  const held = count('held');
  const blocked = count('blocked');
  const runnable = tasks.filter(t => t.status !== 'held' && t.status !== 'blocked');
  const done = runnable.filter(t => t.status === 'done').length;
  const pct = runnable.length ? Math.round((done / runnable.length) * 100) : 0;
  const subline = [held ? `${held} held` : '', blocked ? `${blocked} blocked` : ''].filter(Boolean).join(' · ');

  if (!state.tasksOpen) {
    container.innerHTML = `
      <div class="task-rail">
        <button type="button" id="btn-tasks-open" class="task-rail-btn" aria-expanded="false" aria-label="Show tasks, ${done} of ${runnable.length} done">
          ${icon('chevronLeft', 16)}
          ${icon('clipboard', 16)}
          <span class="task-rail-count">${done}/${runnable.length}</span>
        </button>
      </div>
    `;
    container.querySelector('#btn-tasks-open').onclick = () => store.setTasksOpen(true);
    return;
  }

  container.innerHTML = `
    <div class="task-panel">
      <div class="task-panel-header">
        <div class="task-panel-title-row">
          ${icon('clipboard', 18)}
          <h2 class="task-panel-title">${state.mode === 'hacker' ? 'Progress' : 'Tasks'}</h2>
          <button type="button" id="btn-tasks-close" class="task-collapse-btn" aria-expanded="true" aria-label="Hide tasks">
            ${icon('chevronRight', 16)}
          </button>
        </div>
        <div class="task-panel-subhead">${done} of ${runnable.length} done</div>
        ${runnable.length ? `
          <div class="progress-track" role="progressbar" aria-label="Tasks done" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${pct}">
            <div class="progress-fill" style="width: ${pct}%;"></div>
          </div>` : ''}
        ${subline ? `<div class="progress-subline">${subline}</div>` : ''}
      </div>

      <div class="task-panel-divider"></div>

      <div class="task-panel-scroll">
      ${reviewable ? renderPlanCard(plan) : ''}
      ${plan.status === 'completed' ? renderCompletedPlan(plan) : ''}
      ${reviewable ? '' : `<ol class="task-list">
        ${tasks.map(t => {
          const [statusIcon, chipClass, label] = STATUS[t.status] || [null, '', 'Waiting'];
          const source = t.sourceDomain || t.source || '';
          return `
            <li class="task-item ${t.status === 'running' ? 'active' : ''}">
              <div class="task-item-left">
                <div class="task-status-icon ${esc(t.status || 'waiting')}" aria-hidden="true">${statusIcon ? icon(statusIcon, 14) : esc(t.num)}</div>
                <div class="task-item-text">
                  <div class="task-item-title">${esc(t.title)}</div>
                  <div class="task-item-meta">
                    ${source ? `<span>${esc(source)}</span>` : ''}
                    ${t.verdict === 'YELLOW' ? '<span class="task-meta-caution">· Caution</span>' : ''}
                    ${t.verdict === 'RED' ? '<span class="task-meta-blocked">· Blocked</span>' : ''}
                  </div>
                </div>
              </div>
              <div class="status-chip-sm ${chipClass}">${label}</div>
            </li>
          `;
        }).join('')}
      </ol>`}
      </div>
    </div>
  `;
  container.querySelector('#btn-tasks-close').onclick = () => store.setTasksOpen(false);
  bindPlanCard(container, () => renderTaskPanel(container));
}
