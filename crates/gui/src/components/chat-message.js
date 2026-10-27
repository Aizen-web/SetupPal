/**
 * SetupPal: one chat message as HTML.
 *
 * Kept free of store/bridge imports so the dev preview page (chat-preview.html)
 * can render exactly what the app renders.
 */

import { formatChatMessage, escapeHtml } from './chat-formatter.js';

const PETS = {
  ghost: { emoji: '👻', name: 'Pixel' },
  cat: { emoji: '🐱', name: 'Byte' },
  bunny: { emoji: '🐰', name: 'Mochi' },
};

/**
 * @param {object} m                  { id, sender, text, time, options? }
 * @param {object} ctx
 * @param {boolean} ctx.isLast        latest message: interactive + shows quick actions
 * @param {boolean} ctx.isNew         animate in
 * @param {string}  ctx.activePet     'ghost' | 'cat' | 'bunny'
 * @param {object}  ctx.ui            { selections, collapsed }
 */
export function renderMessage(m, ctx) {
  const { isLast = false, isNew = false, activePet = 'ghost', ui = {} } = ctx || {};
  const pet = PETS[activePet] || PETS.ghost;
  const idAttr = escapeHtml(m.id);
  const newCls = isNew ? ' is-new' : '';

  if (m.sender === 'user') {
    return `
      <div class="chat-bubble user-bubble${newCls}" data-msg-id="${idAttr}">
        <div class="user-text">${escapeHtml(m.text)}</div>
        <span class="chat-timestamp">${escapeHtml(m.time)}</span>
      </div>
    `;
  }

  // Only the latest assistant message is interactive; older choice forms are read-only.
  const body = formatChatMessage(m.text, {
    msgId: m.id,
    interactive: isLast,
    selections: ui.selections,
    collapsed: ui.collapsed,
  });

  const options = isLast && m.options && m.options.length > 0 ? `
    <div class="chat-options-container">
      <div class="chat-options-header">Quick actions</div>
      <div class="chat-options-group">
        ${m.options.map(opt => `
          <button type="button" class="chat-option-pill ${opt.value === 'open_settings' ? 'pill-settings' : ''}" data-choice="${escapeHtml(opt.value)}">
            <span class="option-label">${escapeHtml(opt.label)}</span>
          </button>
        `).join('')}
      </div>
    </div>
  ` : '';

  return `
    <div class="chat-bubble bot-bubble${newCls}" data-msg-id="${idAttr}">
      <div class="chat-avatar-wrap" aria-hidden="true">${pet.emoji}</div>
      <div class="chat-msg-col">
        <div class="chat-meta">
          <span class="chat-author-name">${pet.name}</span>
          <span class="chat-timestamp">${escapeHtml(m.time)}</span>
        </div>
        <div class="chat-bubble-body">${body}</div>
        ${options}
      </div>
    </div>
  `;
}
