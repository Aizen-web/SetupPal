/**
 * SetupPal: History View
 * Real data only: backend transaction log + locally saved conversations.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { bridge } from '../api/bridge.js';
import { icon } from './icon.js';

function esc(value) {
  return String(value ?? '').replace(/[&<>"']/g, ch => (
    { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]
  ));
}

function formatWhen(ts) {
  if (!ts) return '';
  const d = new Date(ts);
  if (isNaN(d.getTime())) return '';
  return d.toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

export function renderHistoryView(container) {
  const state = store.getState();
  const { transactions, loaded, error } = state.history;
  const ordered = [...transactions].reverse();
  const latestRewindable = ordered.find(tx => !tx.rolledBackTxId && !tx.dryRun && tx.rollbackAvailable);

  const conversations = [...(state.conversations || [])]
    .filter(c => (c.chatMessages || []).length > 0)
    .sort((a, b) => (b.updatedAt || 0) - (a.updatedAt || 0));

  let changesHtml;
  if (error) {
    changesHtml = `<div class="history-empty">Could not load the change log: ${esc(error)}</div>`;
  } else if (!loaded) {
    changesHtml = `<div class="history-empty">Loading change log...</div>`;
  } else if (ordered.length === 0) {
    changesHtml = `<div class="history-empty">No changes recorded yet. Approved installs will show up here.</div>`;
  } else {
    changesHtml = ordered.map(tx => {
      const isRollback = !!tx.rolledBackTxId;
      const canRewind = !isRollback && !tx.dryRun && tx.rollbackAvailable;
      const tag = isRollback ? `Rewind of ${esc(tx.rolledBackTxId)}` : tx.dryRun ? 'Dry run' : tx.rollbackAvailable ? 'Reversible' : 'Not reversible';
      return `
        <div style="padding: 10px 14px; background: rgba(255,255,255,0.4); border: 1px solid var(--border-subtle); border-radius: var(--radius-md);">
          <div style="display: flex; align-items: center; justify-content: space-between; gap: 12px;">
            <div style="display: flex; align-items: center; gap: 12px; min-width: 0;">
              <span class="font-mono" style="font-size: 11px; color: var(--ink-soft); white-space: nowrap;">${esc(formatWhen(tx.timestamp))}</span>
              <span style="font-weight: 600; font-size: 13.5px; color: var(--ink);">${esc(tx.summary)}</span>
            </div>
            <div style="display: flex; align-items: center; gap: 10px; flex-shrink: 0;">
              <span style="font-size: 11px; background: var(--accent-soft); color: var(--accent-strong); padding: 2px 8px; border-radius: 999px;">${tag}</span>
              ${canRewind ? `<button class="btn-review btn-rewind-tx" data-tx="${esc(tx.id)}" style="padding: 3px 8px; font-size: 11px;">Rewind</button>` : ''}
            </div>
          </div>
          ${tx.goal ? `<div style="font-size: 12px; color: var(--ink-soft); margin-top: 4px;">Goal: ${esc(tx.goal)}</div>` : ''}
          ${tx.changes.length ? `<ul class="font-mono" style="margin: 6px 0 0 16px; padding: 0; font-size: 11px; color: var(--ink-soft);">${tx.changes.map(c => `<li>${esc(c)}</li>`).join('')}</ul>` : ''}
        </div>`;
    }).join('');
  }

  const convHtml = conversations.length === 0
    ? `<div class="history-empty">No conversations yet. Start chatting and they will be saved here.</div>`
    : conversations.map(c => {
        const msgs = c.chatMessages || [];
        const userCount = msgs.filter(m => m.sender === 'user').length;
        const plan = c.activePlan;
        const taskCount = plan?.tasks?.length || 0;
        const doneCount = (plan?.tasks || []).filter(t => t.status === 'done').length;
        const meta = [
          `${userCount} message${userCount === 1 ? '' : 's'}`,
          taskCount ? `${doneCount} of ${taskCount} tasks done` : null,
          c.updatedAt ? formatWhen(c.updatedAt) : (c.createdAt || null)
        ].filter(Boolean).join(' · ');
        const isActive = c.id === state.activeConversationId;
        return `
          <div style="display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 14px; border-radius: var(--radius-md); border: 1px solid var(--border-subtle);">
            <div style="min-width: 0;">
              <div style="font-weight: 600; font-size: 14px; color: var(--ink);">${esc(c.icon || '')} ${esc(c.title)}${isActive ? ' <span style="font-size: 11px; color: var(--accent-strong);">(open)</span>' : ''}</div>
              <div style="font-size: 12px; color: var(--ink-soft);">${esc(meta)}</div>
            </div>
            <div style="display: flex; gap: 8px; flex-shrink: 0;">
              <button class="btn-review btn-open-conv" data-conv="${esc(c.id)}" style="padding: 3px 10px; font-size: 11px;">Open</button>
              <button class="btn-review btn-delete-conv" data-conv="${esc(c.id)}" style="padding: 3px 10px; font-size: 11px; border-color: var(--v-red); color: var(--v-red);">Delete</button>
            </div>
          </div>`;
      }).join('');

  container.innerHTML = `
    <div class="main-content" style="padding: 24px 32px; overflow-y: auto;">
      <div style="margin-bottom: 24px;">
        <h1 class="font-pixel" style="font-size: 24px; color: var(--ink);">History & Rewind Log</h1>
        <div style="font-size: 13px; color: var(--ink-soft); margin-top: 4px;">
          Audit log of every change made to this machine, plus your saved conversations.
        </div>
      </div>

      <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; margin-bottom: 24px; box-shadow: var(--shadow-card);">
        <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px;">
          <h2 class="font-pixel" style="font-size: 18px; color: var(--ink);">Changes</h2>
          <div style="display: flex; align-items: center; gap: 8px;">
            <a id="btn-export-audit" href="/api/history/export?format=html" target="_blank" class="btn-review" style="font-size: 12.5px; text-decoration: none; display: flex; align-items: center; gap: 4px; padding: 5px 12px;">
              ${icon('fileText', 18)} Export Audit Report (HTML)
            </a>
            <button id="btn-rewind" class="btn-review" ${latestRewindable ? '' : 'disabled'} style="border-color: var(--v-red); color: var(--v-red); font-size: 12.5px;">
              ${icon('rewind', 18)} Rewind Last Change
            </button>
          </div>
        </div>
        <div style="display: flex; flex-direction: column; gap: 10px;">${changesHtml}</div>
      </div>

      <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
        <h2 class="font-pixel" style="font-size: 18px; color: var(--ink); margin-bottom: 14px;">Past Conversations</h2>
        <div style="display: flex; flex-direction: column; gap: 8px;">${convHtml}</div>
      </div>
    </div>
  `;

  container.querySelector('#btn-rewind').onclick = () => {
    if (latestRewindable) bridge.rewindLastChange(latestRewindable.id);
  };
  container.querySelectorAll('.btn-rewind-tx').forEach(btn => {
    btn.onclick = () => bridge.rewindLastChange(btn.dataset.tx);
  });
  container.querySelectorAll('.btn-open-conv').forEach(btn => {
    btn.onclick = () => {
      if (btn.dataset.conv === store.getState().activeConversationId) {
        store.setActivePage('chats');
      } else {
        store.switchConversation(btn.dataset.conv);
      }
    };
  });
  container.querySelectorAll('.btn-delete-conv').forEach(btn => {
    btn.onclick = () => {
      sounds.play('ui.click');
      store.closeConversation(btn.dataset.conv);
    };
  });
}
