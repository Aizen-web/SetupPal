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
 * Splits a message into `{ text, thinking }`, including legacy stored messages where the provider
 * leaked its chain-of-thought into `m.text` or wrapped it in `<think>…</think>`.
 */
export function splitMessageThinking(m) {
  let text = String(m?.text || '');
  let thinking = String(m?.thinking || '').trim();

  // 1. Explicit <think>…</think> tags inside text
  const tagMatch = text.match(/^\s*<think>([\s\S]*?)(?:<\/think>\s*|$)/i);
  if (tagMatch) {
    const inside = tagMatch[1].trim();
    if (inside) thinking = thinking ? `${thinking}\n\n${inside}` : inside;
    text = text.slice(tagMatch[0].length);
  } else if (text.includes('</think>')) {
    const idx = text.indexOf('</think>');
    const inside = text.slice(0, idx).trim();
    if (inside) thinking = thinking ? `${thinking}\n\n${inside}` : inside;
    text = text.slice(idx + '</think>'.length);
  }

  // 2. Legacy leaked "Here's a thinking process:" trace in stored messages
  if (!thinking && /^\s*Here's a thinking process:/i.test(text)) {
    const trimmed = text.trim();
    const sentenceMatch = trimmed.match(/\n(?:Sentence|Response|Reply|Final (?:response|answer)):\s*"([^"\n]+)"[\s\S]*$/i);
    if (sentenceMatch) {
      thinking = trimmed;
      text = sentenceMatch[1].trim();
    } else {
      thinking = trimmed;
      text = '';
    }
  }

  // Strip any leftover inline ``setuppal-intent ...`` from legacy leaked traces
  text = text.replace(/`+setuppal-intent\s*\{[\s\S]*?\}`+/gi, '').trim();

  return { text, thinking };
}

function renderThinkingDisclosure(thinking, { msgId, isStreaming, hasVisibleText, collapsed }) {
  if (!thinking) return '';
  const key = `${msgId}:thinking`;
  // Collapsed by default so only the output and plan show unless clicked open.
  const isOpen = collapsed instanceof Set ? collapsed.has(`${key}:open`) : false;
  const label = isStreaming && !hasVisibleText ? 'Thinking…' : 'Thinking';
  return `
    <details class="chat-thinking" data-thinking-key="${esc(`${key}:open`)}"${isOpen ? ' open' : ''}>
      <summary class="chat-thinking-summary">
        <span class="chat-thinking-chev" aria-hidden="true">${icon('chevronRight', 13)}</span>
        <span class="chat-thinking-label">${label}</span>
      </summary>
      <div class="chat-thinking-body">${esc(thinking)}</div>
    </details>
  `;
}

/**
 * @param {object} m                  { id, sender, text, thinking?, time, options?, isStreaming?, stopped?, failed? }
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

  const { text: cleanText, thinking } = splitMessageThinking(m);

  // Only the latest assistant message is interactive; older choice forms are read-only.
  const body = formatChatMessage(cleanText, {
    msgId: m.id,
    interactive: isLast,
    selections: ui.selections,
    collapsed: ui.collapsed,
  });
  const thinkingHtml = renderThinkingDisclosure(thinking, {
    msgId: m.id,
    isStreaming: Boolean(m.isStreaming),
    hasVisibleText: Boolean(cleanText),
    collapsed: ui.collapsed,
  });
  const streaming = m.isStreaming ? ' is-streaming' : '';
  const ariaBusy = m.isStreaming ? ' aria-busy="true"' : '';
  const streamIndicator = m.isStreaming
    ? (cleanText ? '<span class="chat-stream-caret" aria-hidden="true"></span>' : '<span class="chat-typing-indicator" role="status" aria-label="Thinking"><i></i><i></i><i></i></span>')
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

  const completed = m.completedPlan?.steps?.length ? `
    <section class="plan-card plan-completed-card" aria-label="Completed plan">
      <div class="plan-card-head">
        <div class="plan-card-head-text">
          <h2 class="plan-title">${icon('check', 16)} Completed: ${esc(m.completedPlan.title)}</h2>
          <p class="plan-subtitle">${m.completedPlan.steps.length} step${m.completedPlan.steps.length === 1 ? '' : 's'} taken</p>
        </div>
      </div>
      <ol class="plan-steps-list">
        ${m.completedPlan.steps.map(s => `
          <li class="step-row"><div class="step-left"><span class="step-number" aria-hidden="true">${esc(s.num)}</span><div class="step-info"><div class="step-title">${esc(s.title)}</div></div></div><span class="badge badge-green">${icon('check', 12)}DONE</span></li>
        `).join('')}
      </ol>
    </section>
  ` : '';

  const failure = m.failed ? ' is-failed' : '';
  const showRetry = isLast && canRetry && retryText && !m.isStreaming;
  const actions = m.isStreaming ? '' : `
    <div class="chat-msg-actions${isLast || m.failed ? ' is-pinned' : ''}" role="group" aria-label="Message actions">
      ${cleanText ? `
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
        ${thinkingHtml}
        <div class="chat-bubble-body">${body}${streamIndicator}</div>
        ${completed}
        ${options}
        ${actions}
      </div>
    </div>
  `;
}

export function updateStreamingMessage(id, text, thinking = '') {
  const bubble = [...document.querySelectorAll('.chat-bubble[data-msg-id]')]
    .find(element => element.getAttribute('data-msg-id') === String(id));
  if (!bubble) return;
  const body = bubble.querySelector('.chat-bubble-body');
  if (!body) return;
  const scrollArea = bubble.closest('.workspace-scroll-area');
  const shouldFollow = scrollArea
    ? scrollArea.scrollHeight - scrollArea.scrollTop - scrollArea.clientHeight < 80
    : false;

  const { text: cleanText, thinking: cleanThinking } = splitMessageThinking({ text, thinking });
  if (cleanThinking) {
    let details = bubble.querySelector('details.chat-thinking');
    const label = !cleanText ? 'Thinking…' : 'Thinking';
    if (!details) {
      body.insertAdjacentHTML('beforebegin', renderThinkingDisclosure(cleanThinking, {
        msgId: id,
        isStreaming: true,
        hasVisibleText: Boolean(cleanText),
      }));
    } else {
      const labelEl = details.querySelector('.chat-thinking-label');
      const bodyEl = details.querySelector('.chat-thinking-body');
      if (labelEl) labelEl.textContent = label;
      if (bodyEl) bodyEl.textContent = cleanThinking;
    }
  }

  const indicator = cleanText
    ? '<span class="chat-stream-caret" aria-hidden="true"></span>'
    : '<span class="chat-typing-indicator" role="status" aria-label="Thinking"><i></i><i></i><i></i></span>';
  body.innerHTML = `${formatChatMessage(cleanText, { msgId: id, interactive: false })}${indicator}`;
  bubble.setAttribute('aria-busy', 'true');
  bubble.classList.add('is-streaming');
  if (scrollArea && shouldFollow) scrollArea.scrollTop = scrollArea.scrollHeight;
}

/** Keep Hacker Mode's Terminal tab alive while the model streams (chat panel is hidden). */
export function updateTerminalModelStream(text) {
  const list = document.getElementById('terminal-log-output');
  if (!list) return;
  list.querySelector('.log-empty')?.remove();
  let line = list.querySelector('[data-model-stream="true"]');
  if (!line) {
    line = document.createElement('div');
    line.className = 'log-line running is-live is-model-stream';
    line.setAttribute('data-model-stream', 'true');
    list.appendChild(line);
  }
  const compact = String(text || '').replace(/\s+/g, ' ').trim();
  line.textContent = !compact
    ? '~ …'
    : compact.length > 220
      ? `~ ${compact.slice(0, 220)}…`
      : `~ ${compact}`;
  const terminal = list.closest('.terminal-container');
  if (terminal) terminal.scrollTop = terminal.scrollHeight;
}
