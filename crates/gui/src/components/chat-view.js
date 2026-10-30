/**
 * SetupPal: Chat & Execution Workspace View
 * Calm empty state (Pixel + headline + starters + composer), a slim active chat with
 * Pixel docked in the header, a quiet plan card, and the Hacker-mode terminal tab.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { simulation } from '../state/simulation.js';
import { bridge } from '../api/bridge.js';
import { pixelVoice } from '../mascot/voicelines.js';
import { createPixelSvg } from '../mascot/pixel.js';
import { renderMessage } from './chat-message.js';
import { esc, safeHttpsUrl } from './html.js';
import { icon } from './icon.js';
import { contextButtonHtml, contextModalHtml, bindContextPanel, isContextOpen } from './context-panel.js';
import { registerSlashCommand, listSlashCommands, matchSlashCommands, runSlashCommand } from './slash-commands.js';

/**
 * UI state that must survive full re-renders (the view rebuilds its DOM on every store update):
 * radio selections, collapsed sections, which messages have already animated in,
 * which plan-step "Why?" rows are expanded, the slash menu, and the composer notice.
 */
const chatUi = {
  selections: {},
  collapsed: new Set(),
  seen: new Set(),
  greetingSeen: false,
  whyOpen: new Set(),
  slashHelp: false,
  slashIndex: 0,
  notice: null, // { text, tone }
};

let noticeTimer = null;

const STARTERS = [
  {
    key: 'web',
    title: 'Web',
    hint: 'Node, TypeScript, Vite',
    prompt: 'set up fullstack Node.js and TypeScript web environment',
    iconName: 'code',
  },
  {
    key: 'python',
    title: 'Python',
    hint: 'venv, PyTorch, Jupyter',
    prompt: 'setup Python AI data science venv',
    iconName: 'package',
  },
  {
    key: 'cpp',
    title: 'C++',
    hint: 'Compilers, CMake',
    prompt: 'set me up for C++20 dev',
    iconName: 'bolt',
  },
  {
    key: 'minecraft',
    title: 'Minecraft',
    hint: 'Fabric mods for Java',
    prompt: 'Modded Minecraft Java with Fabric',
    iconName: 'package',
  },
  {
    key: 'everyday',
    title: 'Everyday',
    hint: '7-Zip, VLC, and Git',
    prompt: '7-Zip, VLC, and Git',
    iconName: 'download',
  },
  {
    key: 'obs',
    title: 'OBS Studio',
    hint: 'Record and stream',
    prompt: 'OBS Studio',
    iconName: 'bolt',
  },
];

const VERDICT_ICON = { GREEN: 'check', YELLOW: 'alert', RED: 'shieldX' };
const VERDICT_CLASS = { GREEN: 'badge-green', YELLOW: 'badge-yellow', RED: 'badge-red' };

/* ------------------------------------------------------------------ */
/* Composer notice + slash commands                                    */
/* ------------------------------------------------------------------ */

function noticeHtml() {
  if (!chatUi.notice) return '';
  return `<span class="composer-notice-text tone-${esc(chatUi.notice.tone)}">${esc(chatUi.notice.text)}</span>`;
}

function notify(text, tone = 'info') {
  chatUi.notice = { text, tone };
  const el = document.getElementById('composer-notice');
  if (el) el.innerHTML = noticeHtml();
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => {
    chatUi.notice = null;
    const current = document.getElementById('composer-notice');
    if (current) current.innerHTML = '';
  }, 6000);
}

function latestRewindable() {
  return [...(store.getState().history.transactions || [])]
    .reverse()
    .find(tx => !tx.rolledBackTxId && !tx.dryRun && tx.rollbackAvailable);
}

registerSlashCommand({
  name: 'new',
  hint: 'Start a new chat',
  run: () => store.createConversation(),
});

registerSlashCommand({
  name: 'undo',
  hint: 'Roll back the last change SetupPal made',
  run: async ({ notify: say }) => {
    if (!store.getState().history.loaded) await bridge.refreshHistory();
    const tx = latestRewindable();
    if (!tx) {
      say('Nothing to undo yet. Changes from approved runs show up in History.');
      return;
    }
    if (!confirm(`Roll back this change on your PC?\n\n${tx.summary}`)) {
      say('Undo cancelled.');
      return;
    }
    say(`Rolling back: ${tx.summary}`);
    const res = await bridge.rewindLastChange(tx.id);
    if (res?.status === 'rolled_back') say(`Rolled back: ${tx.summary}`, 'ok');
    else say(`Undo did not finish: ${res?.message || 'see History for details.'}`, 'error');
  },
});

registerSlashCommand({
  name: 'help',
  hint: 'Show these commands',
  run: () => {
    chatUi.slashHelp = true;
    chatUi.slashIndex = 0;
  },
});

registerSlashCommand({
  name: 'hack',
  hint: 'Switch to Hacker mode',
  run: () => {
    store.setModeManual('hacker');
    pixelVoice.modeChanged('hacker');
  },
});

registerSlashCommand({
  name: 'chill',
  hint: 'Switch to Chill mode',
  run: () => {
    store.setModeManual('chill');
    pixelVoice.modeChanged('chill');
  },
});

function slashMenuItems(value) {
  if (chatUi.slashHelp && !value) return listSlashCommands();
  return matchSlashCommands(value);
}

function slashMenuHtml(items) {
  if (!items.length) return '';
  const index = Math.min(chatUi.slashIndex, items.length - 1);
  return `
    <div class="slash-menu-title">Commands</div>
    ${items.map((c, i) => `
      <button type="button" class="slash-item ${i === index ? 'is-active' : ''}" role="option" aria-selected="${i === index}" data-slash="${esc(c.name)}" tabindex="-1">
        <span class="slash-name">/${esc(c.name)}</span>
        <span class="slash-hint">${esc(c.hint)}</span>
      </button>
    `).join('')}
  `;
}

/* ------------------------------------------------------------------ */
/* Pieces                                                              */
/* ------------------------------------------------------------------ */

function flashCopied(btn) {
  btn.classList.add('copied');
  const label = btn.querySelector('span');
  const original = label ? label.textContent : '';
  if (label) label.textContent = 'Copied';
  setTimeout(() => {
    btn.classList.remove('copied');
    if (label) label.textContent = original;
  }, 1500);
}

function verdictBadge(verdict) {
  const v = String(verdict || '').toUpperCase();
  const cls = VERDICT_CLASS[v] || '';
  const ico = VERDICT_ICON[v];
  return `<span class="badge ${cls}">${ico ? icon(ico, 12) : ''}${esc(v || 'UNRATED')}</span>`;
}

function renderEmptyState() {
  return `
    <div class="welcome-block">
      <h2 class="welcome-title">What should we set up?</h2>
      <p class="welcome-sub">Runtimes, dev tools, and apps that install unattended from the command line.</p>

      <div class="starter-pills" role="group" aria-label="Suggestions">
        ${STARTERS.map(s => `
          <button type="button" class="starter-pill" data-prompt="${esc(s.prompt)}">
            <span class="starter-pill-icon" aria-hidden="true">${icon(s.iconName, 16)}</span>
            <span class="starter-pill-copy">
              <span class="starter-pill-title">${esc(s.title)}</span>
              <span class="starter-pill-hint">${esc(s.hint)}</span>
            </span>
          </button>
        `).join('')}
      </div>
    </div>
  `;
}

function renderPlanCard(plan) {
  if (!plan?.tasks?.length) return '';

  const review = plan.tasks.filter(t => t.verdict === 'YELLOW' || t.verdict === 'RED').length;
  const meta = `${plan.tasks.length} step${plan.tasks.length === 1 ? '' : 's'}${review ? ` · ${review} need${review === 1 ? 's' : ''} a look` : ''}`;

  return `
    <section class="plan-card" aria-labelledby="plan-card-title">
      <div class="plan-card-head">
        <div class="plan-card-head-text">
          <h2 class="plan-title" id="plan-card-title">${esc(plan.title)}</h2>
          ${plan.subtitle ? `<p class="plan-subtitle">${esc(plan.subtitle)}</p>` : ''}
        </div>
        <span class="plan-meta">${esc(meta)}</span>
      </div>

      <ol class="plan-steps-list">
        ${plan.tasks.map(t => {
          const source = t.sourceDomain || t.source || '';
          const href = safeHttpsUrl(t.sourceUrl);
          const reasons = Array.isArray(t.reasons) && t.reasons.length ? t.reasons : (t.verdictDesc ? [t.verdictDesc] : []);
          const showWhy = (t.verdict === 'YELLOW' || t.verdict === 'RED') && (reasons.length || t.verdictTitle);
          const whyOpen = chatUi.whyOpen.has(String(t.id));
          const whyId = `why-${esc(t.id)}`;

          return `
            <li class="step-row" data-task-id="${esc(t.id)}">
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
                </div>
              </div>
              ${verdictBadge(t.verdict)}
            </li>
          `;
        }).join('')}
      </ol>

      <div class="plan-actions-row">
        <button id="btn-approve-plan" class="btn-approve" type="button">Approve safe steps</button>
        ${review ? `<button id="btn-review-yellow" class="btn-review-link" type="button">Review warnings</button>` : ''}
      </div>

      <label class="plan-trust-row" for="trust-winget-checkbox">
        <input type="checkbox" id="trust-winget-checkbox" />
        <span>Don't ask again about verified winget packages</span>
      </label>
    </section>
  `;
}

function renderTerminal(state, isWorking) {
  const logs = state.terminalLogs || [];
  const plan = state.activePlan;
  return `
    <div class="terminal-container" id="panel-terminal" role="tabpanel" aria-labelledby="tab-terminal">
      <div class="terminal-header">
        <div class="terminal-dots" aria-hidden="true">
          <span class="dot dot-red"></span>
          <span class="dot dot-yellow"></span>
          <span class="dot dot-green"></span>
        </div>
        <div class="terminal-title">Run log${plan?.title ? ` · ${esc(plan.title)}` : ''}</div>
      </div>
      <div class="terminal-log-list" id="terminal-log-output" role="log" aria-label="Run log">
        ${logs.length === 0 ? `
          <div class="log-line log-empty">No run output yet. Approve a plan and each step's output streams here.</div>
        ` : logs.map(l => `<div class="log-line ${esc(l.type)}">${esc(l.text)}</div>`).join('')}
        ${isWorking ? `
          <div class="log-line" aria-hidden="true">
            <span class="terminal-prompt">› </span><span class="terminal-cursor"></span>
          </div>` : ''}
      </div>
    </div>
  `;
}

function speechBubbleHtml(state, { compact }) {
  if (!state.settings.speechBubbles) return '';
  const text = state.mascotSpeech || pixelVoice.line('welcome');
  if (compact) {
    return `<div class="speech-bubble is-compact"><span id="speech-bubble-text" class="speech-bubble-inner-text">${esc(text)}</span></div>`;
  }
  return `
    <div class="speech-bubble ${chatUi.greetingSeen ? '' : 'is-new'}">
      <div class="speech-bubble-tail" aria-hidden="true"></div>
      <div id="speech-bubble-text" class="speech-bubble-inner-text">${esc(text)}</div>
    </div>
  `;
}

/* ------------------------------------------------------------------ */
/* View                                                                */
/* ------------------------------------------------------------------ */

export function renderChatView(container) {
  const state = store.getState();
  const plan = state.activePlan;
  const isHacker = state.mode === 'hacker';
  const isWorking = Boolean(plan?.tasks?.some((task) => task.status === 'running'));
  const isGenerating = Boolean(state.isChatGenerating);
  const hasMessages = Boolean(state.chatMessages && state.chatMessages.length > 0);
  const showTerminal = isHacker && state.terminalTab === 'terminal';

  // Preserve scroll position (the DOM is rebuilt below)
  const prevScrollEl = container.querySelector('.workspace-scroll-area');
  const prevScrollTop = prevScrollEl ? prevScrollEl.scrollTop : 0;
  const wasAtBottom = prevScrollEl
    ? prevScrollEl.scrollHeight - prevScrollEl.scrollTop - prevScrollEl.clientHeight < 80
    : true;

  const messages = state.chatMessages || [];
  const lastMsg = messages[messages.length - 1];
  const firstRender = chatUi.seen.size === 0;
  const newMsgIds = messages.filter(m => !chatUi.seen.has(m.id)).map(m => m.id);
  messages.forEach(m => chatUi.seen.add(m.id));

  const prevInput = container.querySelector('#chat-input');
  const prevValue = prevInput ? prevInput.value : '';

  const placeholder = isGenerating
    ? 'Ask another question while Pixel replies…'
    : isWorking ? 'Working… press Esc to stop' : 'What would you like to set up?';

  const menuItems = slashMenuItems(prevValue);

  container.innerHTML = `
    <div class="main-content chat-view ${hasMessages ? 'has-messages' : ''}">
      <header class="top-nav-bar">
        <div class="top-nav-left">
          ${hasMessages ? `
            <button type="button" id="hero-mascot" class="mascot-button is-docked" aria-label="Poke Pixel">
              ${createPixelSvg(36, 'header', { live: true })}
            </button>
            ${speechBubbleHtml(state, { compact: true })}
          ` : ''}
        </div>
        ${isHacker ? `
          <div class="hacker-tab-strip" role="tablist" aria-label="Workspace view">
            <button type="button" id="tab-terminal" class="hacker-tab-btn ${showTerminal ? 'active' : ''}" role="tab" aria-selected="${showTerminal}" aria-controls="panel-terminal" data-tab="terminal">Terminal</button>
            <button type="button" id="tab-chat" class="hacker-tab-btn ${showTerminal ? '' : 'active'}" role="tab" aria-selected="${!showTerminal}" aria-controls="panel-chat" data-tab="chat">Chat</button>
          </div>
        ` : ''}
        <div class="top-nav-right">
          ${contextButtonHtml()}
          <div class="mode-toggle-group" role="group" aria-label="Theme mode">
            <button id="toggle-chill" type="button" class="mode-btn ${!isHacker ? 'active' : ''}" aria-pressed="${!isHacker}">
              ${icon('leaf', 14)}<span class="mode-btn-label">Chill</span>
            </button>
            <button id="toggle-hacker" type="button" class="mode-btn ${isHacker ? 'active' : ''}" aria-pressed="${isHacker}">
              ${icon('terminal', 14)}<span class="mode-btn-label">Hacker</span>
            </button>
          </div>
        </div>
      </header>

      ${hasMessages ? '' : `
        <div class="mascot-hero-section">
          <button type="button" id="hero-mascot" class="mascot-button" aria-label="Poke Pixel">
            ${createPixelSvg(140, 'hero', { live: true })}
          </button>
          ${speechBubbleHtml(state, { compact: false })}
        </div>
      `}

      <div class="workspace-scroll-area">
        ${showTerminal ? renderTerminal(state, isWorking) : `
          <div class="chat-panel" ${isHacker ? 'id="panel-chat" role="tabpanel" aria-labelledby="tab-chat"' : ''}>
            ${hasMessages ? `
              <div class="chat-thread-stream" id="chat-thread-stream">
                ${messages.map((m, index) => renderMessage(m, {
                  isLast: m === lastMsg,
                  isNew: !firstRender && newMsgIds.includes(m.id),
                  retryText: messages[index - 1]?.sender === 'user' ? messages[index - 1].text : '',
                  canRetry: !isGenerating,
                  ui: chatUi,
                })).join('')}
              </div>
            ` : renderEmptyState()}
            ${renderPlanCard(plan)}
          </div>
        `}
      </div>

      <div class="bottom-bar">
        <div id="slash-menu" class="slash-menu" role="listbox" aria-label="Commands" ${menuItems.length ? '' : 'hidden'}>${slashMenuHtml(menuItems)}</div>
        <div class="composer-box">
          <button type="button" id="btn-slash-hint" class="composer-slash" aria-label="Show commands" aria-controls="slash-menu" aria-expanded="${menuItems.length > 0}">/</button>
          <label for="chat-input" class="sr-only">Message</label>
          <textarea id="chat-input" class="composer-input" rows="1" placeholder="${esc(placeholder)}" aria-describedby="composer-notice"></textarea>
          ${isWorking || isGenerating ? `<button id="btn-stop-run" class="btn-stop" type="button" aria-label="${isGenerating ? 'Stop response' : 'Stop task'}">Stop</button>` : ''}
          <button id="btn-chat-send" class="btn-send" type="button" aria-label="Send">
            <span class="btn-send-label">Send</span>
            ${icon('send')}
          </button>
        </div>
        <div id="composer-notice" class="composer-notice" role="status" aria-live="polite">${noticeHtml()}</div>
        ${hasMessages ? '' : `<p class="composer-scope">SetupPal installs things that set up unattended from the command line. Anything else gets a step-by-step manual guide.</p>`}
      </div>
    </div>
  `;
  chatUi.greetingSeen = true;

  if (isContextOpen()) container.insertAdjacentHTML('beforeend', contextModalHtml());
  bindContextPanel(container, () => renderChatView(container));

  const mascotSvg = container.querySelector('[data-mascot-live="true"]');
  if (mascotSvg) {
    mascotSvg.dataset.state = state.mascotState;
    mascotSvg.dataset.mode = state.mode;
  }

  bindHeader(container);
  bindPlanCard(container, plan);
  bindMessages(container);
  bindComposer(container, prevValue);
  restoreScroll(container, { newMsgIds, firstRender, wasAtBottom, prevScrollTop });
}

/* ------------------------------------------------------------------ */
/* Bindings                                                            */
/* ------------------------------------------------------------------ */

function bindHeader(container) {
  container.querySelector('#toggle-chill').onclick = () => {
    sounds.play('ui.click');
    store.setModeManual('chill');
    pixelVoice.modeChanged('chill');
  };
  container.querySelector('#toggle-hacker').onclick = () => {
    sounds.play('ui.click');
    store.setModeManual('hacker');
    pixelVoice.modeChanged('hacker');
  };

  const tabs = [...container.querySelectorAll('.hacker-tab-btn')];
  tabs.forEach((tab, i) => {
    tab.onclick = () => store.setTerminalTab(tab.dataset.tab);
    tab.onkeydown = (e) => {
      if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
      e.preventDefault();
      const next = tabs[(i + (e.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length];
      store.setTerminalTab(next.dataset.tab);
      document.getElementById(next.id)?.focus();
    };
  });

  const mascot = container.querySelector('#hero-mascot');
  if (mascot) {
    mascot.onclick = () => {
      pixelVoice.trigger('poke');
      setTimeout(() => {
        if (store.getState().mascotState === 'celebrate') store.setMascotState('idle');
      }, 2400);
    };
  }
}

function bindPlanCard(container, plan) {
  const btnApprove = container.querySelector('#btn-approve-plan');
  if (btnApprove) {
    btnApprove.onclick = () => {
      const trustWinget = container.querySelector('#trust-winget-checkbox')?.checked;
      const categories = trustWinget ? ['verified_winget'] : [];
      bridge.approveAndRunPlan(categories);
    };
  }

  // Expand every YELLOW/RED "Why?" row
  const btnReview = container.querySelector('#btn-review-yellow');
  if (btnReview) {
    btnReview.onclick = () => {
      const held = (plan.tasks || []).filter(t => t.verdict === 'YELLOW' || t.verdict === 'RED');
      held.forEach(t => chatUi.whyOpen.add(String(t.id)));
      if (held.length) pixelVoice.trigger('warning_held');
      sounds.play('ui.click');
      renderChatView(container);
    };
  }

  container.querySelectorAll('.step-why-btn').forEach(btn => {
    btn.onclick = () => {
      const id = btn.getAttribute('data-why');
      if (chatUi.whyOpen.has(id)) chatUi.whyOpen.delete(id);
      else chatUi.whyOpen.add(id);
      sounds.play('ui.click');
      renderChatView(container);
    };
  });
}

async function copyText(btn, text) {
  try {
    await navigator.clipboard.writeText(text);
    flashCopied(btn);
    sounds.play('ui.click');
  } catch (err) {
    console.warn('Could not copy to clipboard:', err);
  }
}

function bindMessages(container) {
  container.querySelectorAll('.chat-action-btn').forEach(btn => {
    btn.onclick = () => {
      if (btn.dataset.action === 'copy') {
        const msg = store.getState().chatMessages.find(m => String(m.id) === btn.dataset.msgId);
        if (msg?.text) copyText(btn, msg.text);
      } else if (btn.dataset.action === 'retry') {
        const prompt = btn.dataset.retryText?.trim();
        if (prompt) bridge.submitGoal(prompt);
      }
    };
  });

  container.querySelectorAll('.starter-pill').forEach(card => {
    card.onclick = () => {
      pixelVoice.trigger('prompt_selected');
      bridge.submitGoal(card.getAttribute('data-prompt'));
    };
  });

  container.querySelectorAll('.chat-option-pill').forEach(btn => {
    btn.onclick = () => {
      const choice = btn.getAttribute('data-choice');
      sounds.play('ui.click');
      if (choice === 'open_settings') {
        store.setActivePage('settings');
        return;
      }
      if (choice === 'open_accounts') {
        store.setActivePage('accounts');
        return;
      }
      bridge.submitGoal(choice);
    };
  });

  container.querySelectorAll('.choice-input').forEach(input => {
    input.onchange = () => {
      chatUi.selections[`${input.dataset.form}:${input.dataset.gi}`] = Number(input.dataset.oi);
      sounds.play('ui.click');
      const form = container.querySelector(`.chat-choice-form[data-form="${input.dataset.form}"]`);
      const summary = container.querySelector(`[data-summary="${input.dataset.form}"]`);
      if (form && summary) {
        summary.textContent = [...form.querySelectorAll('.choice-input:checked')].map(i => i.value).join(' · ');
      }
    };
  });

  container.querySelectorAll('.chat-continue-btn').forEach(btn => {
    btn.onclick = () => {
      const formId = btn.getAttribute('data-continue');
      const form = container.querySelector(`.chat-choice-form[data-form="${formId}"]`);
      if (!form) return;
      const parts = [];
      form.querySelectorAll('.choice-group[data-name]').forEach(group => {
        const checked = group.querySelector('.choice-input:checked');
        if (checked) parts.push(`${group.getAttribute('data-name')}: ${checked.value}`);
      });
      if (parts.length === 0) return;
      btn.disabled = true;
      sounds.play('ui.click');
      bridge.submitGoal(parts.join(' · '));
    };
  });

  container.querySelectorAll('details.chat-sys').forEach(details => {
    details.addEventListener('toggle', () => {
      const key = details.getAttribute('data-key');
      if (details.open) chatUi.collapsed.delete(key);
      else chatUi.collapsed.add(key);
    });
  });

  container.querySelectorAll('.sys-copy').forEach(btn => {
    btn.onclick = (e) => {
      e.stopPropagation();
      const value = btn.getAttribute('data-copy');
      if (value) copyText(btn, value);
    };
  });

  container.querySelectorAll('.chat-code-copy-btn').forEach(btn => {
    btn.onclick = (e) => {
      e.stopPropagation();
      const code = btn.getAttribute('data-code');
      if (code) copyText(btn, code);
    };
  });
}

const supportsFieldSizing = typeof CSS !== 'undefined' && CSS.supports?.('field-sizing', 'content');

function autoSize(textarea) {
  if (supportsFieldSizing) return;
  textarea.style.height = 'auto';
  textarea.style.height = `${textarea.scrollHeight}px`;
}

function bindComposer(container, prevValue) {
  const input = container.querySelector('#chat-input');
  const btnSend = container.querySelector('#btn-chat-send');
  const btnStop = container.querySelector('#btn-stop-run');
  const menu = container.querySelector('#slash-menu');
  const hint = container.querySelector('#btn-slash-hint');

  if (prevValue && !input.value) input.value = prevValue;
  autoSize(input);

  const refreshMenu = () => {
    const items = slashMenuItems(input.value);
    menu.innerHTML = slashMenuHtml(items);
    menu.hidden = items.length === 0;
    hint.setAttribute('aria-expanded', String(items.length > 0));
    menu.querySelectorAll('.slash-item').forEach(item => {
      item.onmousedown = (e) => e.preventDefault();
      item.onclick = () => runCommand(`/${item.dataset.slash}`);
    });
    return items;
  };

  const closeMenu = () => {
    chatUi.slashHelp = false;
    chatUi.slashIndex = 0;
    refreshMenu();
  };

  const runCommand = async (text) => {
    input.value = '';
    autoSize(input);
    chatUi.slashHelp = false;
    chatUi.slashIndex = 0;
    refreshMenu();
    sounds.play('ui.click');
    await runSlashCommand(text, { notify });
    if (chatUi.slashHelp) {
      refreshMenu();
      document.getElementById('chat-input')?.focus();
    }
  };

  const send = async () => {
    const text = input.value.trim();
    if (!text) return;
    input.value = '';
    autoSize(input);
    closeMenu();
    const handled = await runSlashCommand(text, { notify });
    if (chatUi.slashHelp) {
      refreshMenu();
      document.getElementById('chat-input')?.focus();
    }
    if (!handled) bridge.submitGoal(text);
  };

  refreshMenu();

  let typingTimer = null;
  input.oninput = () => {
    chatUi.slashIndex = 0;
    if (input.value) chatUi.slashHelp = false;
    refreshMenu();
    autoSize(input);
    const root = container.querySelector('[data-mascot-live="true"]');
    if (root) root.dataset.state = 'listening';
    clearTimeout(typingTimer);
    typingTimer = setTimeout(() => {
      if (root && root.dataset.state === 'listening') root.dataset.state = store.getState().mascotState || 'idle';
    }, 1500);
  };

  input.onkeydown = (e) => {
    const items = menu.hidden ? [] : slashMenuItems(input.value);
    if (items.length) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const step = e.key === 'ArrowDown' ? 1 : items.length - 1;
        chatUi.slashIndex = (Math.min(chatUi.slashIndex, items.length - 1) + step) % items.length;
        refreshMenu();
        return;
      }
      if (e.key === 'Tab') {
        e.preventDefault();
        input.value = `/${items[Math.min(chatUi.slashIndex, items.length - 1)].name}`;
        chatUi.slashHelp = false;
        refreshMenu();
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        closeMenu();
        return;
      }
      if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
        e.preventDefault();
        runCommand(`/${items[Math.min(chatUi.slashIndex, items.length - 1)].name}`);
        return;
      }
    }
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      send();
    }
  };

  hint.onclick = () => {
    chatUi.slashHelp = !chatUi.slashHelp || Boolean(input.value);
    chatUi.slashIndex = 0;
    if (chatUi.slashHelp) input.value = '';
    refreshMenu();
    input.focus();
  };

  btnSend.onclick = send;

  if (btnStop) btnStop.onclick = () => {
    if (store.getState().isChatGenerating) bridge.stopChat();
    else simulation.stopRun();
  };
}

function restoreScroll(container, { newMsgIds, firstRender, wasAtBottom, prevScrollTop }) {
  const scrollArea = container.querySelector('.workspace-scroll-area');
  if (!scrollArea) return;

  const terminal = container.querySelector('.terminal-container');
  if (terminal) {
    requestAnimationFrame(() => { terminal.scrollTop = terminal.scrollHeight; });
    return;
  }

  const chatStream = container.querySelector('#chat-thread-stream');
  if (!chatStream) return;
  requestAnimationFrame(() => {
    const newEls = newMsgIds
      .map(id => [...chatStream.querySelectorAll('[data-msg-id]')].find(el => el.getAttribute('data-msg-id') === String(id)))
      .filter(Boolean);
    const newest = newEls[newEls.length - 1];

    if (firstRender) {
      scrollArea.scrollTop = scrollArea.scrollHeight;
    } else if (newest && newest.classList.contains('bot-bubble')) {
      const offset = newest.getBoundingClientRect().top - scrollArea.getBoundingClientRect().top;
      const isTall = newest.offsetHeight > scrollArea.clientHeight - 24;
      scrollArea.scrollTop = isTall
        ? scrollArea.scrollTop + offset - 12
        : scrollArea.scrollHeight;
    } else if (newest) {
      scrollArea.scrollTop = scrollArea.scrollHeight;
    } else {
      scrollArea.scrollTop = wasAtBottom ? scrollArea.scrollHeight : prevScrollTop;
    }
  });
}
