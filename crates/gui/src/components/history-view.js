/**
 * SetupPal: History View
 * Real data only: backend transaction log + locally saved conversations.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { bridge } from '../api/bridge.js';
import { esc } from './html.js';
import { icon } from './icon.js';

// Search text survives re-renders while the app is open.
let query = '';

function formatWhen(ts) {
  if (!ts) return '';
  const d = new Date(ts);
  if (isNaN(d.getTime())) return '';
  return d.toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

function matches(q, ...fields) {
  if (!q) return true;
  return fields.some(f => String(f ?? '').toLowerCase().includes(q));
}

function txTag(tx, undoneAtById = {}) {
  // Undo rows themselves: no action (R16).
  if (tx.rolledBackTxId) return ['Undo record', 'chip-tag chip-muted'];
  if (tx.dryRun) return ['Dry run', 'chip-tag'];
  if (undoneAtById[tx.id]) {
    return [`Undone at ${undoneAtById[tx.id]}`, 'chip-tag chip-muted'];
  }
  return tx.rollbackAvailable
    ? ['Reversible', 'chip-tag']
    : ['Not reversible', 'chip-tag chip-muted'];
}

function buildBodies() {
  const state = store.getState();
  const { transactions, loaded, error } = state.history;
  const q = query.trim().toLowerCase();
  const ordered = [...transactions].reverse();
  const undoneAtById = {};
  for (const tx of transactions) {
    if (tx.rolledBackTxId) {
      undoneAtById[tx.rolledBackTxId] = formatWhen(tx.timestamp).replace(/^.*,\s*/, '') || formatWhen(tx.timestamp);
    }
  }
  const latestRewindable = ordered.find(tx => !tx.rolledBackTxId && !tx.dryRun && tx.rollbackAvailable && !undoneAtById[tx.id]);
  const shownTx = ordered.filter(tx => matches(q, tx.summary, tx.goal, tx.stepId, ...(tx.changes || [])));

  const conversations = [...(state.conversations || [])]
    .map(c => (c.id === state.activeConversationId ? { ...c, chatMessages: state.chatMessages, activePlan: state.activePlan } : c))
    .filter(c => (c.chatMessages || []).length > 0)
    .sort((a, b) => (b.updatedAt || 0) - (a.updatedAt || 0));
  const shownConv = conversations.filter(c => matches(q, c.title, ...(c.chatMessages || []).map(m => m.text)));

  let changesHtml;
  if (error) {
    changesHtml = `<p class="empty-note">Could not load the change log: ${esc(error)}</p>`;
  } else if (!loaded) {
    changesHtml = `<p class="empty-note">Loading change log…</p>`;
  } else if (ordered.length === 0) {
    changesHtml = `<p class="empty-note">No changes recorded yet. Approved installs will show up here.</p>`;
  } else if (shownTx.length === 0) {
    changesHtml = `<p class="empty-note">No changes match “${esc(query.trim())}”.</p>`;
  } else {
    changesHtml = `<ul class="list-stack">${shownTx.map(tx => {
      const canRewind = !tx.rolledBackTxId && !tx.dryRun && tx.rollbackAvailable && !undoneAtById[tx.id];
      const [tag, tagClass] = txTag(tx, undoneAtById);
      return `
        <li class="list-row list-row-col">
          <div class="list-row-main">
            <div class="list-row-lead">
              <span class="font-mono list-time">${esc(formatWhen(tx.timestamp))}</span>
              <span class="list-title">${esc(tx.summary)}</span>
            </div>
            <div class="btn-row">
              <span class="${tagClass}" ${tx.rolledBackTxId ? `title="Undo of ${esc(tx.rolledBackTxId)}"` : ''}>${tag}</span>
              ${canRewind ? `<button type="button" class="btn-review btn-xs btn-rewind-tx" data-tx="${esc(tx.id)}">Rewind</button>` : ''}
            </div>
          </div>
          ${tx.goal ? `<div class="list-meta">Goal: ${esc(tx.goal)}</div>` : ''}
          ${tx.changes.length ? `<ul class="font-mono change-list">${tx.changes.map(c => `<li>${esc(c)}</li>`).join('')}</ul>` : ''}
        </li>`;
    }).join('')}</ul>`;
  }

  let convHtml;
  if (conversations.length === 0) {
    convHtml = `<p class="empty-note">No conversations yet. Start chatting and they will be saved here.</p>`;
  } else if (shownConv.length === 0) {
    convHtml = `<p class="empty-note">No conversations match “${esc(query.trim())}”.</p>`;
  } else {
    convHtml = `<ul class="list-stack">${shownConv.map(c => {
      const msgs = c.chatMessages || [];
      const userCount = msgs.filter(m => m.sender === 'user').length;
      const tasks = c.activePlan?.tasks || [];
      const doneCount = tasks.filter(t => t.status === 'done').length;
      const meta = [
        `${userCount} message${userCount === 1 ? '' : 's'}`,
        tasks.length ? `${doneCount} of ${tasks.length} tasks done` : null,
        formatWhen(c.updatedAt || c.createdAt) || null,
      ].filter(Boolean).join(' · ');
      const isActive = c.id === state.activeConversationId;
      return `
        <li class="list-row">
          <div class="list-row-text">
            <div class="list-title">${esc(c.title)}${isActive ? ' <span class="list-open-tag">(open)</span>' : ''}</div>
            <div class="list-meta">${esc(meta)}</div>
          </div>
          <div class="btn-row">
            <button type="button" class="btn-review btn-xs btn-open-conv" data-conv="${esc(c.id)}" aria-label="Open ${esc(c.title)}">Open</button>
            <button type="button" class="btn-review btn-xs btn-danger btn-delete-conv" data-conv="${esc(c.id)}" aria-label="Delete ${esc(c.title)}">Delete</button>
          </div>
        </li>`;
    }).join('')}</ul>`;
  }

  return { changesHtml, convHtml, latestRewindable };
}

function confirmRewind(tx) {
  if (tx && confirm(`Roll back this change on your PC?\n\n${tx.summary}`)) bridge.rewindLastChange(tx.id);
}

function findTx(id) {
  return store.getState().history.transactions.find(tx => tx.id === id);
}

function bindRows(container) {
  container.querySelectorAll('.btn-rewind-tx').forEach(btn => {
    btn.onclick = () => confirmRewind(findTx(btn.dataset.tx));
  });
  container.querySelectorAll('.btn-open-conv').forEach(btn => {
    btn.onclick = () => {
      if (btn.dataset.conv === store.getState().activeConversationId) store.setActivePage('chats');
      else store.switchConversation(btn.dataset.conv);
    };
  });
  container.querySelectorAll('.btn-delete-conv').forEach(btn => {
    btn.onclick = () => {
      sounds.play('ui.click');
      store.closeConversation(btn.dataset.conv);
    };
  });
}

/** Search only swaps the result lists, so the input keeps focus and every keystroke. */
function refreshResults(container) {
  const { changesHtml, convHtml } = buildBodies();
  container.querySelector('#history-changes-body').innerHTML = changesHtml;
  container.querySelector('#history-conv-body').innerHTML = convHtml;
  bindRows(container);
}

export function renderHistoryView(container) {
  const { changesHtml, convHtml, latestRewindable } = buildBodies();
  const prevScroll = container.querySelector('.page')?.scrollTop || 0;
  container.innerHTML = `
    <div class="main-content page">
      <div class="page-inner page-inner-wide">
        <header class="page-header page-header-row">
          <div>
            <h1 class="page-title">History</h1>
            <p class="page-sub">Every change made to this PC, plus your saved conversations.</p>
          </div>
          <div class="search-field">
            ${icon('search', 16)}
            <label for="history-search" class="sr-only">Search history</label>
            <input id="history-search" class="search-input" type="search" placeholder="Search changes and chats" value="${esc(query)}" autocomplete="off" />
          </div>
        </header>

        <div class="page-stack">
          <section class="card" aria-labelledby="history-changes-title">
            <div class="card-head">
              <h2 class="card-title" id="history-changes-title">Changes</h2>
              <div class="btn-row">
                <a id="btn-export-audit" href="/api/history/export?format=html" target="_blank" rel="noopener" class="btn-review btn-sm">
                  ${icon('fileText', 16)} Export audit report
                </a>
                <button type="button" id="btn-rewind" class="btn-review btn-sm btn-danger" ${latestRewindable ? '' : 'disabled'}>
                  ${icon('rewind', 16)} Rewind last change
                </button>
              </div>
            </div>
            <div id="history-changes-body">${changesHtml}</div>
          </section>

          <section class="card" aria-labelledby="history-conv-title">
            <h2 class="card-title" id="history-conv-title">Past conversations</h2>
            <div id="history-conv-body">${convHtml}</div>
          </section>
        </div>
      </div>
    </div>
  `;
  const page = container.querySelector('.page');
  if (page) page.scrollTop = prevScroll;

  const search = container.querySelector('#history-search');
  search.oninput = () => {
    query = search.value;
    refreshResults(container);
  };
  search.onkeydown = (e) => {
    if (e.key === 'Escape' && search.value) {
      e.preventDefault();
      e.stopPropagation();
      search.value = '';
      query = '';
      refreshResults(container);
    }
  };

  container.querySelector('#btn-rewind').onclick = () => confirmRewind(latestRewindable);
  bindRows(container);
}
