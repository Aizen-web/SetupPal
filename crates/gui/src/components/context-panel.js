/**
 * SetupPal: Context window viewer.
 * Shows what was sent to the model on the last turn: token breakdown, compacted memory notes,
 * and the recent messages kept word for word.
 */

import { getLastContext } from '../state/prefs.js';

let open = false;
export const isContextOpen = () => open;
export const setContextOpen = (v) => { open = Boolean(v); };

const esc = (s) => String(s ?? '').replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

const PARTS = [
  ['system_tokens', 'Instructions', '#7c6cf0'],
  ['memory_tokens', 'Memory notes', '#e0a030'],
  ['recent_tokens', 'Recent messages', '#3fa37a'],
  ['facts_tokens', 'PC facts', '#4a9ad9'],
  ['user_tokens', 'Your message', '#d9604a'],
];

export function contextButtonHtml() {
  const ctx = getLastContext();
  const label = ctx ? `Context · ~${ctx.total_tokens} tokens` : 'Context';
  return `<button id="btn-context" class="btn-review" type="button" style="padding: 6px 12px; font-size: 12px;" title="See what the AI is given each turn">${esc(label)}</button>`;
}

export function contextModalHtml() {
  if (!open) return '';
  const ctx = getLastContext();
  const card = 'background: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); box-shadow: var(--shadow-card);';

  let body;
  if (!ctx) {
    body = `<div style="font-size: 13px; color: var(--ink-soft); padding: 12px 0;">Nothing sent yet. Send a message and the exact context will show up here.</div>`;
  } else {
    const total = Math.max(ctx.total_tokens, 1);
    const bar = PARTS.map(([k, , c]) => `<div style="width: ${(ctx[k] / total) * 100}%; background: ${c};"></div>`).join('');
    const legend = PARTS.map(([k, name, c]) => `
      <div style="display: flex; align-items: center; justify-content: space-between; font-size: 12.5px; padding: 3px 0;">
        <span><span style="display: inline-block; width: 9px; height: 9px; border-radius: 2px; background: ${c}; margin-right: 8px;"></span>${name}</span>
        <span class="font-mono" style="color: var(--ink-soft);">~${ctx[k]}</span>
      </div>`).join('');
    const recent = (ctx.recent || []).map(m => `
      <div style="font-size: 12px; margin-bottom: 6px;"><strong>${m.role === 'user' ? 'You' : 'Pixel'}:</strong> <span style="color: var(--ink-soft);">${esc(m.content)}</span></div>`).join('');

    body = `
      <div style="display: flex; height: 10px; border-radius: 6px; overflow: hidden; background: var(--border-subtle); margin: 4px 0 10px;">${bar}</div>
      ${legend}
      <div style="display: flex; justify-content: space-between; font-size: 12.5px; font-weight: 600; border-top: 1px solid var(--border-subtle); margin-top: 6px; padding-top: 6px;">
        <span>Sent to model</span><span class="font-mono">~${ctx.total_tokens} tokens</span>
      </div>
      <div style="font-size: 11.5px; color: var(--ink-soft); margin-top: 2px;">Reply limit: ${ctx.reply_budget} tokens · Model: ${esc(ctx.model)}</div>

      <div style="font-size: 12px; color: var(--ink-soft); margin: 14px 0 6px;">
        ${ctx.total_messages} messages in chat: last ${ctx.recent_messages} sent as written, ${ctx.compacted_messages} older ones folded into notes.
      </div>

      <details style="margin-bottom: 8px;" ${ctx.memory_note ? 'open' : ''}>
        <summary style="cursor: pointer; font-weight: 600; font-size: 13px;">Memory notes</summary>
        <pre style="white-space: pre-wrap; font-size: 11.5px; margin: 6px 0 0; color: var(--ink-soft);">${ctx.memory_note ? esc(ctx.memory_note) : 'None yet. The chat is short enough to send in full.'}</pre>
      </details>
      <details style="margin-bottom: 8px;">
        <summary style="cursor: pointer; font-weight: 600; font-size: 13px;">Recent messages (${(ctx.recent || []).length})</summary>
        <div style="margin-top: 6px;">${recent || '<span style="font-size: 12px; color: var(--ink-soft);">None</span>'}</div>
      </details>
      <details>
        <summary style="cursor: pointer; font-weight: 600; font-size: 13px;">PC facts sent</summary>
        <pre style="white-space: pre-wrap; font-size: 11.5px; margin: 6px 0 0; color: var(--ink-soft);">${esc(ctx.facts)}</pre>
      </details>`;
  }

  return `
    <div id="context-overlay" style="position: fixed; inset: 0; background: rgba(0,0,0,0.35); display: flex; align-items: center; justify-content: center; z-index: 1000;">
      <div style="${card} width: min(520px, 92vw); max-height: 82vh; overflow-y: auto; padding: 20px 24px;">
        <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px;">
          <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin: 0;">Context window</h2>
          <button id="btn-context-close" class="btn-review" type="button" style="padding: 4px 10px; font-size: 12px;">Close</button>
        </div>
        ${body}
      </div>
    </div>`;
}

/** Wire the button and modal inside a freshly rendered container. */
export function bindContextPanel(container, rerender) {
  const btn = container.querySelector('#btn-context');
  if (btn) btn.onclick = () => { open = true; rerender(); };
  const close = () => { open = false; rerender(); };
  const closeBtn = container.querySelector('#btn-context-close');
  if (closeBtn) closeBtn.onclick = close;
  const overlay = container.querySelector('#context-overlay');
  if (overlay) overlay.onclick = (e) => { if (e.target === overlay) close(); };
}
