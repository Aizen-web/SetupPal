/**
 * SetupPal: one chat message as HTML.
 *
 * Kept free of store/bridge imports so it renders the same wherever it is used.
 */

import { formatChatMessage } from './chat-formatter.js';
import { esc } from './html.js';
import { icon } from './icon.js';

export const ASSISTANT_NAME = 'Pixel';

/**
 * @param {object} m                  { id, sender, text, time, options?, isStreaming?, stopped?, failed? }
 * @param {object} ctx
 * @param {boolean} ctx.isLast        latest message: interactive and keeps its actions visible
 * @param {boolean} ctx.isNew         animate in
 * @param {string}  ctx.retryText     preceding user prompt, enables Retry on the latest reply
 * @param {boolean} ctx.canRetry      false while another reply is streaming
 * @param {object}  ctx.ui            { selections, collapsed }
 */
export function renderMessage(m, ctx) {
  const { isLast = false, isNew = false, retryText = '', canRetry = true, ui = {} } = ctx || {};
  const idAttr = esc(m.id);
  const newCls = isNew ? ' is-new' : '';

  if (m.sender === 'user') {
    return `
      <div class="chat-bubble user-bubble${newCls}" data-msg-id="${idAttr}">
        <div class="user-text">${esc(m.text)}</div>
        <span class="chat-timestamp">${esc(m.time)}</span>
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
          <button type="button" class="chat-option-pill ${opt.value === 'open_settings' ? 'pill-settings' : ''}" data-choice="${esc(opt.value)}">
            <span class="option-label">${esc(opt.label)}</span>
          </button>
        `).join('')}
      </div>
    </div>
  ` : '';

  const failure = m.failed ? ' is-failed' : '';
  const showRetry = isLast && canRetry && retryText && !m.isStreaming;
  const actions = m.isStreaming ? '' : `
    <div class="chat-msg-actions${isLast || m.failed ? ' is-pinned' : ''}" role="group" aria-label="Message actions">
      ${m.text ? `
        <button type="button" class="chat-action-btn" data-action="copy" data-msg-id="${idAttr}" aria-label="Copy reply">
          ${icon('copy', 14)}<span>Copy</span>
        </button>` : ''}
      ${showRetry ? `
        <button type="button" class="chat-action-btn" data-action="retry" data-retry-text="${esc(retryText)}" aria-label="${m.failed ? 'Try again' : 'Retry this question'}">
          ${icon('refresh', 14)}<span>${m.failed ? 'Try again' : 'Retry'}</span>
        </button>` : ''}
    </div>
  `;

  return `
    <div class="chat-bubble bot-bubble${newCls}${streaming}${failure}" data-msg-id="${idAttr}"${ariaBusy}>
      <div class="chat-avatar-wrap" aria-hidden="true">${icon('ghost', 20)}</div>
      <div class="chat-msg-col">
        <div class="chat-meta">
          <span class="chat-author-name">${ASSISTANT_NAME}</span>
          ${m.stopped ? '<span class="chat-status-label">Stopped</span>' : ''}
          <span class="chat-timestamp">${esc(m.time)}</span>
        </div>
        <div class="chat-bubble-body">${body}${streamIndicator}</div>
        ${options}
        ${actions}
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
