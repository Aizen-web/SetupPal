/**
 * SetupPal: Chat & Execution Workspace View
 * Calm empty state (Pixel + headline + starters + composer),
 * slim active chat, and a quiet plan card.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { simulation } from '../state/simulation.js';
import { bridge } from '../api/bridge.js';
import { petVoiceEngine } from '../mascot/voicelines.js';
import { createMascotSvg } from '../mascot/pets.js';
import { PETS, renderMessage } from './chat-message.js';
import { icon } from './icon.js';
import { contextButtonHtml, contextModalHtml, bindContextPanel, isContextOpen } from './context-panel.js';

/**
 * UI state that must survive full re-renders (the view rebuilds its DOM on every store update):
 * radio selections, collapsed sections, which messages have already animated in,
 * and which plan-step "Why?" rows are expanded.
 */
const chatUi = {
  selections: {},
  collapsed: new Set(),
  seen: new Set(),
  greetingSeen: false,
  whyOpen: new Set(),
};

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
];

function updateChoiceSummary(container, formId) {
  const form = container.querySelector(`.chat-choice-form[data-form="${formId}"]`);
  const summary = container.querySelector(`[data-summary="${formId}"]`);
  if (!form || !summary) return;
  const values = [...form.querySelectorAll('.choice-input:checked')].map(i => i.value);
  summary.textContent = values.join(' · ');
}

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

function leafSvg() {
  return `
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M11 20A7 7 0 0 1 9.8 6.1C15.5 5 17 4.48 19 2c1 2 2 4.18 2 8 0 5.5-4.78 10-10 10Z"></path>
      <path d="M2 21c0-3 1.85-5.36 5.08-6C9.5 14.52 12 13 13 12"></path>
    </svg>
  `;
}

function termPromptSvg() {
  return `
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="4 17 10 11 4 5"></polyline>
      <line x1="12" y1="19" x2="20" y2="19"></line>
    </svg>
  `;
}

function renderEmptyState() {
  return `
    <div class="welcome-block">
      <h2 class="welcome-title">What should we set up?</h2>
      <p class="welcome-sub">Web stacks, runtimes, and coding tools — quiet and unattended.</p>

      <div class="starter-pills" role="list">
        ${STARTERS.map(s => `
          <button type="button" class="starter-pill" data-prompt="${s.prompt}" role="listitem">
            <span class="starter-pill-icon" aria-hidden="true">${icon(s.iconName, 16)}</span>
            <span class="starter-pill-copy">
              <span class="starter-pill-title">${s.title}</span>
              <span class="starter-pill-hint">${s.hint}</span>
            </span>
          </button>
        `).join('')}
      </div>
    </div>
  `;
}

function renderPlanCard(plan) {
  if (!plan?.tasks?.length) return '';

  const hasWarnings = plan.tasks.some(t => t.verdict === 'YELLOW' || t.verdict === 'RED');

  return `
    <div class="plan-card">
      <div class="plan-title">${plan.title}</div>
      <div class="plan-subtitle">${plan.subtitle}</div>

      <div class="plan-steps-list">
        ${plan.tasks.map(t => {
          const badgeClass = t.verdict === 'GREEN' ? 'badge-green'
            : t.verdict === 'YELLOW' ? 'badge-yellow' : 'badge-red';
          const source = t.sourceDomain || t.source || '';
          const showWhy = t.verdict === 'YELLOW' || t.verdict === 'RED';
          const whyOpen = chatUi.whyOpen.has(t.id);
          const reasons = (t.reasons && t.reasons.length > 0)
            ? t.reasons
            : (t.verdictDesc ? [t.verdictDesc] : ['Needs a closer look before we install.']);

          return `
            <div class="step-row" data-task-id="${t.id}">
              <div class="step-left">
                <div class="step-number">${t.num}</div>
                <div class="step-info">
                  <div class="step-title">${t.title}</div>
                  <div class="step-source">${source}</div>
                  ${showWhy ? `
                    <button type="button" class="step-why-btn" data-why="${t.id}" aria-expanded="${whyOpen}">
                      Why?
                    </button>
                    <div class="step-why-detail ${whyOpen ? 'is-open' : ''}" data-why-detail="${t.id}">
                      <strong>${t.verdictTitle || t.verdict}</strong>
                      <ul>${reasons.map(r => `<li>${r}</li>`).join('')}</ul>
                    </div>
                  ` : ''}
                </div>
              </div>
              <div class="badge ${badgeClass}">${t.verdict}</div>
            </div>
          `;
        }).join('')}
      </div>

      <div class="plan-actions-row">
        <button id="btn-approve-plan" class="btn-approve" type="button">Approve safe steps</button>
        ${hasWarnings ? `<button id="btn-review-yellow" class="btn-review-link" type="button">Review warnings</button>` : ''}
      </div>

      <label class="plan-trust-row" for="trust-winget-checkbox">
        <input type="checkbox" id="trust-winget-checkbox" />
        <span>Don't ask again about verified winget packages</span>
      </label>
    </div>
  `;
}

export function renderChatView(container) {
  const state = store.getState();
  const plan = state.activePlan;
  const isHacker = state.mode === 'hacker';
  const activePet = PETS[state.activePet] ? state.activePet : 'ghost';
  const pet = PETS[activePet];
  const isWorking = Boolean(plan?.tasks?.some((task) => task.status === 'running'));
  const isGenerating = Boolean(state.isChatGenerating);
  const hasMessages = Boolean(state.chatMessages && state.chatMessages.length > 0);

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
  const hadFocus = prevInput && document.activeElement === prevInput;
  const prevValue = prevInput ? prevInput.value : '';
  const selStart = prevInput ? prevInput.selectionStart : 0;
  const selEnd = prevInput ? prevInput.selectionEnd : 0;

  const petSize = hasMessages ? 40 : 140;
  const petPrefix = hasMessages ? 'header' : 'hero';

  container.innerHTML = `
    <div class="main-content ${hasMessages ? 'has-messages' : ''}">
      <div class="top-nav-bar">
        ${contextButtonHtml()}
        <div class="top-nav-spacer" aria-hidden="true"></div>
        <label class="pet-select-wrap">
          <span class="sr-only">Choose chat mascot</span>
          <select id="select-chat-pet" class="pet-select" aria-label="Choose chat mascot">
            ${Object.entries(PETS).map(([id, identity]) => `
              <option value="${id}" ${activePet === id ? 'selected' : ''}>${identity.name} · ${identity.style}</option>
            `).join('')}
          </select>
        </label>
        <div class="mode-toggle-group" title="Switch Chill / Hacker">
          <button id="toggle-chill" class="mode-btn ${!isHacker ? 'active' : ''}" aria-label="Chill mode">
            ${leafSvg()}
            <span class="mode-btn-label">Chill</span>
          </button>
          <button id="toggle-hacker" class="mode-btn ${isHacker ? 'active' : ''}" aria-label="Hacker mode">
            ${termPromptSvg()}
            <span class="mode-btn-label">Hacker</span>
          </button>
        </div>
      </div>

      <div class="mascot-hero-section ${hasMessages ? 'is-compact' : ''}">
        <div id="hero-mascot" class="mascot-container" title="Click to interact with ${pet.name}!">
          ${createMascotSvg(activePet, petSize, petPrefix)}
        </div>
        ${hasMessages ? '' : `
          <div class="speech-bubble ${chatUi.greetingSeen ? '' : 'is-new'}">
            <div class="speech-bubble-tail"></div>
            <div id="speech-bubble-text" class="speech-bubble-inner-text">${state.mascotSpeech || petVoiceEngine.line('welcome')}</div>
          </div>
        `}
      </div>

      <div class="workspace-scroll-area">
        ${isHacker && state.terminalTab === 'terminal' ? `
          <div class="terminal-container">
            <div class="terminal-header">
              <div class="terminal-dots">
                <span class="dot dot-red"></span>
                <span class="dot dot-yellow"></span>
                <span class="dot dot-green"></span>
              </div>
              <div>zero-trust-worker // sandbox-proc-4291 [pet: ${activePet.toUpperCase()}]</div>
            </div>
            <div class="terminal-log-list" id="terminal-log-output">
              ${state.terminalLogs.map(l => `
                <div class="log-line ${l.type}">${l.text}</div>
              `).join('')}
              <div class="log-line">
                <span class="terminal-prompt">› </span>
                <span class="terminal-cursor"></span>
              </div>
            </div>
          </div>
        ` : `
          ${hasMessages ? `
            <div class="chat-thread-stream" id="chat-thread-stream">
              ${messages.map((m, index) => renderMessage(m, {
                isLast: m === lastMsg,
                isNew: !firstRender && newMsgIds.includes(m.id),
                activePet,
                retryText: messages[index - 1]?.sender === 'user' ? messages[index - 1].text : '',
                ui: chatUi,
              })).join('')}
            </div>
          ` : renderEmptyState()}

          ${renderPlanCard(plan)}
        `}
      </div>

      <div class="bottom-bar">
        <div class="composer-box">
          <input id="chat-input" class="composer-input" type="text" placeholder="${isGenerating ? 'Ask another question while Pixel replies…' : isWorking ? 'Working... press Esc to stop' : 'What would you like to set up?'}" />
          ${isWorking || isGenerating ? `<button id="btn-stop-run" class="btn-stop" type="button" aria-label="${isGenerating ? 'Stop response' : 'Stop task'}">Stop</button>` : ''}
          <button id="btn-chat-send" class="btn-send" type="button" aria-label="Send">
            <span class="btn-send-label">Send</span>
            ${icon('send')}
          </button>
        </div>
        ${hasMessages ? '' : `<p class="composer-scope">Installs only tools that can set up from the command line. No game launchers or apps that need a login.</p>`}
      </div>
    </div>
  `;
  chatUi.greetingSeen = true;

  if (isContextOpen()) container.insertAdjacentHTML('beforeend', contextModalHtml());
  bindContextPanel(container, () => renderChatView(container));

  const mascotSvg = container.querySelector('#hero-ghost-root') || container.querySelector('#header-ghost-root');
  if (mascotSvg) {
    mascotSvg.dataset.state = state.mascotState;
    mascotSvg.dataset.mode = state.mode;
  }

  container.querySelector('#toggle-chill').onclick = () => {
    store.setMode('chill');
    petVoiceEngine.trigger('mode_toggle', { mode: 'chill' });
  };
  container.querySelector('#toggle-hacker').onclick = () => {
    store.setMode('hacker');
    petVoiceEngine.trigger('mode_toggle', { mode: 'hacker' });
  };

  container.querySelector('#select-chat-pet').onchange = (event) => {
    store.setActivePet(event.target.value);
  };

  const heroMascot = container.querySelector('#hero-mascot');
  if (heroMascot) {
    heroMascot.onclick = () => {
      petVoiceEngine.trigger('poke');
      setTimeout(() => {
        if (store.getState().mascotState === 'celebrate') {
          store.setMascotState('idle');
        }
      }, 2400);
    };
  }

  const btnApprove = container.querySelector('#btn-approve-plan');
  if (btnApprove) {
    btnApprove.onclick = () => {
      const trustWinget = container.querySelector('#trust-winget-checkbox')?.checked;
      const categories = trustWinget ? ['verified_winget'] : [];
      petVoiceEngine.trigger('plan_approve');
      bridge.approveAndRunPlan(categories);
    };
  }

  // Expand all YELLOW/RED Why? rows instead of alert()
  const btnReview = container.querySelector('#btn-review-yellow');
  if (btnReview) {
    btnReview.onclick = () => {
      const held = (plan.tasks || []).filter(t => t.verdict === 'YELLOW' || t.verdict === 'RED');
      held.forEach(t => chatUi.whyOpen.add(t.id));
      if (held[0]) petVoiceEngine.trigger('warning_held', { title: held[0].title });
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

  const chatInput = container.querySelector('#chat-input');
  const btnSend = container.querySelector('#btn-chat-send');
  const btnStop = container.querySelector('#btn-stop-run');
  container.querySelectorAll('.chat-retry-btn').forEach((button) => {
    button.onclick = () => {
      const prompt = button.getAttribute('data-retry-text')?.trim();
      if (prompt) bridge.submitGoal(prompt);
    };
  });
  if (btnStop) btnStop.onclick = () => {
    if (store.getState().isChatGenerating) bridge.stopChat();
    else simulation.stopRun();
  };

  if (chatInput) {
    if (prevValue && !chatInput.value) {
      chatInput.value = prevValue;
    }
    if (hadFocus) {
      chatInput.focus();
      try {
        chatInput.setSelectionRange(selStart, selEnd);
      } catch (e) {}
    }

    let typingTimer = null;
    chatInput.oninput = () => {
      const root = container.querySelector('#hero-ghost-root') || container.querySelector('#header-ghost-root');
      if (root) root.dataset.state = 'listening';
      clearTimeout(typingTimer);
      typingTimer = setTimeout(() => {
        if (root && root.dataset.state === 'listening') {
          root.dataset.state = store.getState().mascotState || 'idle';
        }
      }, 1500);
    };
  }

  const handleSend = () => {
    const text = chatInput.value.trim();
    if (!text) return;
    chatInput.value = '';

    if (text.toLowerCase() === '/hack') {
      store.setMode('hacker');
      petVoiceEngine.trigger('mode_toggle', { mode: 'hacker' });
      return;
    }
    if (text.toLowerCase() === '/chill') {
      store.setMode('chill');
      petVoiceEngine.trigger('mode_toggle', { mode: 'chill' });
      return;
    }

    bridge.submitGoal(text);
  };

  if (btnSend && chatInput) {
    btnSend.onclick = handleSend;
    chatInput.onkeydown = (e) => {
      if (e.key === 'Enter') handleSend();
    };
  }

  container.querySelectorAll('.starter-pill').forEach(card => {
    card.onclick = () => {
      const prompt = card.getAttribute('data-prompt');
      petVoiceEngine.trigger('prompt_selected', { prompt });
      bridge.submitGoal(prompt);
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
      updateChoiceSummary(container, input.dataset.form);
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
    btn.onclick = async (e) => {
      e.stopPropagation();
      const value = btn.getAttribute('data-copy');
      if (!value) return;
      try {
        await navigator.clipboard.writeText(value);
        btn.classList.add('copied');
        sounds.play('ui.click');
        setTimeout(() => btn.classList.remove('copied'), 1500);
      } catch (err) {
        console.warn('Could not copy path to clipboard:', err);
      }
    };
  });

  container.querySelectorAll('.chat-code-copy-btn').forEach(btn => {
    btn.onclick = async (e) => {
      e.stopPropagation();
      const code = btn.getAttribute('data-code');
      if (!code) return;
      try {
        await navigator.clipboard.writeText(code);
        flashCopied(btn);
        sounds.play('ui.click');
      } catch (err) {
        console.warn('Could not copy code to clipboard:', err);
      }
    };
  });

  const scrollArea = container.querySelector('.workspace-scroll-area');
  const chatStream = container.querySelector('#chat-thread-stream');
  if (scrollArea && chatStream) {
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
}
