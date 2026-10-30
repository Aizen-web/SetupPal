/**
 * SetupPal: the plan itself, shown in the Tasks panel.
 * Review, approve, start, and remove all happen here; the chat only gets a read-only
 * "Completed plan" record once a run finishes.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { bridge } from '../api/bridge.js';
import { pickProjectFolder } from '../api/folder-picker.js';
import { pixelVoice } from '../mascot/voicelines.js';
import { esc, safeHttpsUrl } from './html.js';
import { icon } from './icon.js';

const VERDICT_ICON = { GREEN: 'check', YELLOW: 'alert', RED: 'shieldX' };
const VERDICT_CLASS = { GREEN: 'badge-green', YELLOW: 'badge-yellow', RED: 'badge-red' };

/** Survives the panel's full re-renders. */
const planUi = {
  whyOpen: new Set(),
  trustWinget: false,
};

function verdictBadge(verdict) {
  const v = String(verdict || '').toUpperCase();
  const cls = VERDICT_CLASS[v] || '';
  const ico = VERDICT_ICON[v];
  return `<span class="badge ${cls}">${ico ? icon(ico, 12) : ''}${esc(v || 'UNRATED')}</span>`;
}

/** Plans that can still be started: not running, not finished. */
export function isPlanReviewable(plan) {
  if (!plan?.tasks?.length) return false;
  return plan.status !== 'completed' && !plan.tasks.some(t => t.status === 'running');
}

export function renderPlanCard(plan) {
  if (!plan?.tasks?.length) return '';

  const review = plan.tasks.filter(t => t.verdict === 'YELLOW' || t.verdict === 'RED').length;
  const approvedCount = plan.tasks.filter(t => {
    if (t.verdict === 'RED') return false;
    if (t.verdict === 'YELLOW') return Boolean(t.approved);
    return t.approved !== false;
  }).length;
  const heldId = plan.haltedAt;
  const totalSteps = plan.tasks.length;
  const meta = `${totalSteps} step${totalSteps === 1 ? '' : 's'}${review ? ` · ${review} need${review === 1 ? 's' : ''} a look` : ''}`;
  const approveLabel = heldId
    ? 'Approve and continue'
    : `Start plan · ${approvedCount} of ${totalSteps} step${totalSteps === 1 ? '' : 's'}`;

  return `
    <section class="plan-card plan-in-panel" aria-labelledby="plan-card-title">
      <div class="plan-card-head">
        <div class="plan-card-head-text">
          <h2 class="plan-title" id="plan-card-title">${esc(plan.title)}</h2>
          ${plan.subtitle ? `<p class="plan-subtitle">${esc(plan.subtitle)}</p>` : ''}
        </div>
        <span class="plan-meta">${plan.dryRun ? '<span class="chip-tag dry-run-badge">Dry run: nothing is installed</span> ' : ''}${esc(meta)}</span>
      </div>

      <ol class="plan-steps-list">
        ${plan.tasks.map(t => {
          const source = t.sourceDomain || t.source || '';
          const href = safeHttpsUrl(t.sourceUrl);
          const reasons = Array.isArray(t.reasons) && t.reasons.length ? t.reasons : (t.verdictDesc ? [t.verdictDesc] : []);
          const showWhy = (t.verdict === 'YELLOW' || t.verdict === 'RED') && (reasons.length || t.verdictTitle);
          const whyOpen = planUi.whyOpen.has(String(t.id));
          const whyId = `why-${esc(t.id)}`;
          const isRed = t.verdict === 'RED';
          const isYellow = t.verdict === 'YELLOW';
          const isGreen = t.verdict === 'GREEN';
          const checked = isGreen ? t.approved !== false : Boolean(t.approved);

          return `
            <li class="step-row ${t.status === 'held' ? 'is-held' : ''}" data-task-id="${esc(t.id)}">
              <div class="step-left">
                <span class="step-number" aria-hidden="true">${esc(t.num)}</span>
                <div class="step-info">
                  <div class="step-title">${esc(t.title)}</div>
                  ${source ? `<div class="step-source">${href
                    ? `<a href="${esc(href)}" target="_blank" rel="noopener noreferrer">${esc(source)}</a>`
                    : esc(source)}</div>` : ''}
                  ${showWhy ? `
                    <button type="button" class="step-why-btn" data-why="${esc(t.id)}" aria-expanded="${whyOpen}" aria-controls="${whyId}">Why?</button>
                    <div class="step-why-detail ${whyOpen ? 'is-open' : ''}" id="${whyId}">
                      ${t.verdictTitle ? `<strong>${esc(t.verdictTitle)}</strong>` : ''}
                      ${reasons.length ? `<ul>${reasons.map(r => `<li>${esc(r)}</li>`).join('')}</ul>` : ''}
                    </div>
                  ` : ''}
                  ${isRed ? `<div class="step-blocked-label">Blocked</div>` : ''}
                  ${isYellow ? `
                    <label class="step-approve-row">
                      <input type="checkbox" class="step-approve-cb" data-step-id="${esc(t.id)}" ${checked ? 'checked' : ''} />
                      <span>Approve this step</span>
                    </label>
                  ` : ''}
                  ${isGreen ? `
                    <label class="step-approve-row">
                      <input type="checkbox" class="step-approve-cb" data-step-id="${esc(t.id)}" ${checked ? 'checked' : ''} />
                      <span>Include this safe step</span>
                    </label>
                  ` : ''}
                </div>
              </div>
              ${verdictBadge(t.verdict)}
            </li>
          `;
        }).join('')}
      </ol>

      ${plan.needsProjectDir ? `
        <div class="plan-folder-row">
          <div class="plan-folder-text">
            <span class="plan-folder-label">Project folder</span>
            <span class="plan-folder-path ${plan.projectDir ? '' : 'is-empty'}" id="plan-folder-path">${plan.projectDir ? esc(plan.projectDir) : 'Pick the folder where npm packages should go'}</span>
          </div>
          <button type="button" class="btn-review-link" id="btn-pick-folder">${plan.projectDir ? 'Change' : 'Choose folder'}</button>
        </div>
      ` : ''}

      <div class="plan-actions-row">
        <button id="btn-approve-plan" class="btn-approve" type="button" ${(approvedCount === 0 && !heldId) || (plan.needsProjectDir && !plan.projectDir) ? 'disabled' : ''}>${esc(approveLabel)}</button>
        ${review ? `<button id="btn-review-yellow" class="btn-review-link" type="button">Review warnings</button>` : ''}
        <button id="btn-remove-plan" class="btn-review-link btn-remove-plan" type="button">Remove plan</button>
      </div>

      <label class="plan-trust-row" for="trust-winget-checkbox">
        <input type="checkbox" id="trust-winget-checkbox" ${planUi.trustWinget ? 'checked' : ''} />
        <span>Don't ask again about verified winget packages</span>
      </label>
    </section>
  `;
}

/** Read-only list of what ran, shown in the panel once the plan is complete. */
export function renderCompletedPlan(plan) {
  const taken = plan.tasks.filter(t => t.status === 'done');
  return `
    <section class="plan-card plan-completed-card" aria-label="Completed plan">
      <div class="plan-card-head">
        <div class="plan-card-head-text">
          <h2 class="plan-title">${icon('check', 16)} Completed: ${esc(plan.title)}</h2>
          <p class="plan-subtitle">${taken.length} step${taken.length === 1 ? '' : 's'} taken</p>
        </div>
      </div>
      <div class="plan-actions-row">
        <button id="btn-remove-plan" class="btn-review-link btn-remove-plan" type="button">Clear</button>
      </div>
    </section>
  `;
}

export function bindPlanCard(container, rerender) {
  container.querySelectorAll('.step-approve-cb').forEach(cb => {
    cb.onchange = () => {
      const id = cb.getAttribute('data-step-id');
      store.setState(s => {
        const tasks = (s.activePlan?.tasks || []).map(t =>
          t.id === id ? { ...t, approved: cb.checked } : t
        );
        return { ...s, activePlan: s.activePlan ? { ...s.activePlan, tasks } : null };
      });
      sounds.play('ui.click');
      rerender?.();
    };
  });

  const trust = container.querySelector('#trust-winget-checkbox');
  if (trust) trust.onchange = () => { planUi.trustWinget = trust.checked; };

  const btnApprove = container.querySelector('#btn-approve-plan');
  if (btnApprove) {
    btnApprove.onclick = () => {
      const plan = store.getState().activePlan;
      const categories = planUi.trustWinget ? ['verified_winget'] : [];
      const tasks = plan?.tasks || [];
      const approvedStepIds = tasks
        .filter(t => t.verdict === 'GREEN' && t.approved !== false)
        .map(t => t.id);
      const allowYellowSteps = tasks
        .filter(t => t.verdict === 'YELLOW' && t.approved)
        .map(t => t.id);
      bridge.approveAndRunPlan({
        trustedCategories: categories,
        approvedStepIds,
        allowYellowSteps,
        startAtStepId: plan?.haltedAt || null,
      });
    };
  }

  const btnRemove = container.querySelector('#btn-remove-plan');
  if (btnRemove) {
    btnRemove.onclick = () => {
      sounds.play('ui.click');
      planUi.whyOpen.clear();
      bridge.removePlan();
    };
  }

  const btnPickFolder = container.querySelector('#btn-pick-folder');
  if (btnPickFolder) {
    btnPickFolder.onclick = async () => {
      sounds.play('ui.click');
      try {
        const folder = await pickProjectFolder();
        if (!folder) return;
        store.setState(s => ({
          ...s,
          activePlan: s.activePlan ? { ...s.activePlan, projectDir: folder } : s.activePlan,
        }));
      } catch (err) {
        store.setState(s => ({
          ...s,
          terminalLogs: [
            ...s.terminalLogs,
            { id: Date.now() + Math.random(), text: `! folder picker failed: ${err?.message || err}`, type: 'error' },
          ],
        }));
      }
    };
  }

  const btnReview = container.querySelector('#btn-review-yellow');
  if (btnReview) {
    btnReview.onclick = () => {
      const plan = store.getState().activePlan;
      const held = (plan?.tasks || []).filter(t => t.verdict === 'YELLOW' || t.verdict === 'RED');
      held.forEach(t => planUi.whyOpen.add(String(t.id)));
      if (held.length) pixelVoice.trigger('warning_held');
      sounds.play('ui.click');
      rerender?.();
    };
  }

  container.querySelectorAll('.step-why-btn').forEach(btn => {
    btn.onclick = () => {
      const id = btn.getAttribute('data-why');
      if (planUi.whyOpen.has(id)) planUi.whyOpen.delete(id);
      else planUi.whyOpen.add(id);
      sounds.play('ui.click');
      rerender?.();
    };
  });
}
