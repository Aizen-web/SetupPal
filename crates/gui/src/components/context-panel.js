/**
 * SetupPal: Context window viewer.
 * Shows what was sent to the model on the last turn: token breakdown, compacted memory notes,
 * and the recent messages kept word for word.
 */

import { getLastContext } from '../state/prefs.js';
import { esc } from './html.js';
import { bindModal, rememberOpener, restoreOpener } from './modal.js';

let open = false;
export const isContextOpen = () => open;
export const setContextOpen = (v) => { open = Boolean(v); };

const PARTS = [
  ['system_tokens', 'Instructions', 'ctx-part-system'],
  ['memory_tokens', 'Memory notes', 'ctx-part-memory'],
  ['recent_tokens', 'Recent messages', 'ctx-part-recent'],
  ['facts_tokens', 'PC facts', 'ctx-part-facts'],
  ['user_tokens', 'Your message', 'ctx-part-user'],
];

const num = (v) => esc(Number(v) || 0);

export function contextButtonHtml() {
  const ctx = getLastContext();
  const label = ctx ? `Context · ~${Number(ctx.total_tokens) || 0} tokens` : 'Context';
  return `<button id="btn-context" class="btn-review btn-sm" type="button" title="See what the AI is given each turn">${esc(label)}</button>`;
}

export function contextModalHtml() {
  if (!open) return '';
  const ctx = getLastContext();

  let body;
  if (!ctx) {
    body = `<p class="ctx-empty">Nothing sent yet. Send a message and the exact context will show up here.</p>`;
  } else {
    const total = Math.max(Number(ctx.total_tokens) || 0, 1);
    // Segment widths are data-driven, so they stay as inline style values.
    const bar = PARTS.map(([k, , cls]) => `<div class="${cls}" style="width: ${((Number(ctx[k]) || 0) / total) * 100}%;"></div>`).join('');
    const legend = PARTS.map(([k, name, cls]) => `
      <div class="ctx-legend-row">
        <span><span class="ctx-swatch ${cls}" aria-hidden="true"></span>${esc(name)}</span>
        <span class="font-mono ctx-muted">~${num(ctx[k])}</span>
      </div>`).join('');
    const recent = (ctx.recent || []).map(m => `
      <div class="ctx-recent-row"><strong>${m.role === 'user' ? 'You' : 'Pixel'}:</strong> <span class="ctx-muted">${esc(m.content)}</span></div>`).join('');

    body = `
      <div class="ctx-bar" role="img" aria-label="Token breakdown">${bar}</div>
      ${legend}
      <div class="ctx-total-row">
        <span>Sent to model</span><span class="font-mono">~${num(ctx.total_tokens)} tokens</span>
      </div>
      <div class="ctx-note">Reply limit: ${num(ctx.reply_budget)} tokens · Model: ${esc(ctx.model)}</div>

      <p class="ctx-summary">
        ${num(ctx.total_messages)} messages in chat: last ${num(ctx.recent_messages)} sent as written, ${num(ctx.compacted_messages)} older ones folded into notes.
      </p>

      <details class="ctx-details" ${ctx.memory_note ? 'open' : ''}>
        <summary>Memory notes</summary>
        <pre class="ctx-pre">${ctx.memory_note ? esc(ctx.memory_note) : 'None yet. The chat is short enough to send in full.'}</pre>
      </details>
      <details class="ctx-details">
        <summary>Recent messages (${(ctx.recent || []).length})</summary>
        <div class="ctx-recent">${recent || '<span class="ctx-muted">None</span>'}</div>
      </details>
      <details class="ctx-details">
        <summary>PC facts sent</summary>
        <pre class="ctx-pre">${esc(ctx.facts)}</pre>
      </details>`;
  }

  return `
    <div id="context-overlay" class="modal-overlay">
      <div class="modal-card ctx-card" role="dialog" aria-modal="true" aria-labelledby="ctx-title" tabindex="-1">
        <div class="modal-head">
          <h2 class="modal-title" id="ctx-title">Context window</h2>
          <button id="btn-context-close" class="btn-review btn-sm" type="button">Close</button>
        </div>
        ${body}
      </div>
    </div>`;
}

/** Wire the button and modal inside a freshly rendered container. */
export function bindContextPanel(container, rerender) {
  const btn = container.querySelector('#btn-context');
  if (btn) btn.onclick = () => { rememberOpener(); open = true; rerender(); };
  const close = () => { open = false; rerender(); restoreOpener(); };
  const closeBtn = container.querySelector('#btn-context-close');
  if (closeBtn) closeBtn.onclick = close;
  const overlay = container.querySelector('#context-overlay');
  if (overlay) bindModal(overlay.querySelector('[role="dialog"]'), { onClose: close, backdrop: overlay });
}
