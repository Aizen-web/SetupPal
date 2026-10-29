/**
 * SetupPal: one chat message as HTML.
 *
 * Kept free of store/bridge imports so the dev preview page (chat-preview.html)
 * can render exactly what the app renders.
 */

import { formatChatMessage, escapeHtml } from './chat-formatter.js';
import { icon } from './icon.js';

export const PETS = {
  ghost: { icon: 'ghost', name: 'Pixel', style: 'playful' },
  cat: { icon: 'cat', name: 'Byte', style: 'dry & grumpy' },
  bunny: { icon: 'bunny', name: 'Mochi', style: 'shy & gentle' },
};

/**
 * @param {object} m                  { id, sender, text, time, options? }
 * @param {object} ctx
 * @param {boolean} ctx.isLast        latest message: interactive + shows quick actions
 * @param {boolean} ctx.isNew         animate in
 * @param {string}  ctx.activePet     'ghost' | 'cat' | 'bunny'
 * @param {string}  ctx.retryText     preceding user prompt for failed replies
 * @param {object}  ctx.ui            { selections, collapsed }
 */
export function renderMessage(m, ctx) {
  const { isLast = false, isNew = false, activePet = 'ghost', retryText = '', ui = {} } = ctx || {};
  const pet = PETS[m.pet || activePet] || PETS.ghost;
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
  const streaming = m.isStreaming ? ' is-streaming' : '';
  const ariaBusy = m.isStreaming ? ' aria-busy="true"' : '';
  const streamIndicator = m.isStreaming
    ? (m.text ? '<span class="chat-stream-caret" aria-hidden="true"></span>' : '<span class="chat-typing-indicator" role="status" aria-label="Thinking"><i></i><i></i><i></i></span>')
    : '';

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
  const failure = m.failed ? ' is-failed' : '';
  const retryAction = m.failed && retryText ? `
    <div class="chat-error-actions">
      <span>Check your connection, then try again.</span>
      <button class="chat-retry-btn" type="button" data-retry-text="${escapeHtml(retryText)}">Try again</button>
    </div>
  ` : '';

  return `
    <div class="chat-bubble bot-bubble${newCls}${streaming}${failure}" data-msg-id="${idAttr}"${ariaBusy}>
      <div class="chat-avatar-wrap" aria-hidden="true">${icon(pet.icon, 20)}</div>
      <div class="chat-msg-col">
        <div class="chat-meta">
          <span class="chat-author-name">${pet.name}</span>
          ${m.stopped ? '<span class="chat-status-label">Stopped</span>' : ''}
          <span class="chat-timestamp">${escapeHtml(m.time)}</span>
        </div>
        <div class="chat-bubble-body">${body}${streamIndicator}</div>
        ${retryAction}
        ${options}
      </div>
    </div>
  `;
}

export function updateStreamingMessage(id, text) {
  const bubble = [...document.querySelectorAll('.chat-bubble[data-msg-id]')]
    .find(element => element.getAttribute('data-msg-id') === String(id));
  if (!bubble) return;
  const body = bubble.querySelector('.chat-bubble-body');
  if (!body) return;
  const scrollArea = bubble.closest('.workspace-scroll-area');
  const shouldFollow = scrollArea
    ? scrollArea.scrollHeight - scrollArea.scrollTop - scrollArea.clientHeight < 80
    : false;
  body.innerHTML = `${formatChatMessage(text, { msgId: id, interactive: false })}<span class="chat-stream-caret" aria-hidden="true"></span>`;
  bubble.setAttribute('aria-busy', 'true');
  bubble.classList.add('is-streaming');
  if (scrollArea && shouldFollow) scrollArea.scrollTop = scrollArea.scrollHeight;
}
