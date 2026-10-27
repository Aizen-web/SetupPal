/**
 * SetupPal: Chat & Execution Workspace View
 * Matching the exact artwork, layout, and copy of the screenshot,
 * with previous animated pets (Pixel the Ghost, Byte the Cat, Mochi the Bunny) restored!
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { simulation } from '../state/simulation.js';
import { bridge } from '../api/bridge.js';
import { petVoiceEngine } from '../mascot/voicelines.js';
import { createMascotSvg } from '../mascot/pets.js';
import { renderMessage } from './chat-message.js';

/**
 * UI state that must survive full re-renders (the view rebuilds its DOM on every store update):
 * radio selections, collapsed sections, and which messages have already animated in.
 */
const chatUi = {
  selections: {},
  collapsed: new Set(),
  seen: new Set(),
};

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

export function renderChatView(container) {
  const state = store.getState();
  const plan = state.activePlan;
  const isHacker = state.mode === 'hacker';
  const activePet = state.activePet || 'ghost';

  // Preserve scroll position (the DOM is rebuilt below)
  const prevScrollEl = container.querySelector('.workspace-scroll-area');
  const prevScrollTop = prevScrollEl ? prevScrollEl.scrollTop : 0;
  const wasAtBottom = prevScrollEl
    ? prevScrollEl.scrollHeight - prevScrollEl.scrollTop - prevScrollEl.clientHeight < 80
    : true;

  // Which messages are new this render (only those animate in / drive scrolling)
  const messages = state.chatMessages || [];
  const lastMsg = messages[messages.length - 1];
  const firstRender = chatUi.seen.size === 0;
  const newMsgIds = messages.filter(m => !chatUi.seen.has(m.id)).map(m => m.id);
  messages.forEach(m => chatUi.seen.add(m.id));

  // Preserve chat input state if user was typing
  const prevInput = container.querySelector('#chat-input');
  const hadFocus = prevInput && document.activeElement === prevInput;
  const prevValue = prevInput ? prevInput.value : '';
  const selStart = prevInput ? prevInput.selectionStart : 0;
  const selEnd = prevInput ? prevInput.selectionEnd : 0;

  // Python Logo SVG
  const pythonLogoSvg = `
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path d="M11.9 2C8.6 2 8.7 3.4 8.7 3.4L8.7 4.9L12.1 4.9L12.1 5.4L5.4 5.4C5.4 5.4 2 5 2 9.5C2 14 5 13.8 5 13.8L6.5 13.8L6.5 11.8C6.5 9.4 8.6 9.5 8.6 9.5L12 9.5C14.4 9.5 14.4 7.6 14.4 7.6L14.4 3.9C14.4 3.9 14.8 2 11.9 2ZM10.2 3.3C10.6 3.3 11 3.6 11 4.1C11 4.5 10.6 4.9 10.2 4.9C9.7 4.9 9.3 4.5 9.3 4.1C9.3 3.6 9.7 3.3 10.2 3.3Z" fill="#387EB8"/>
      <path d="M12.1 22C15.4 22 15.3 20.6 15.3 20.6L15.3 19.1L11.9 19.1L11.9 18.6L18.6 18.6C18.6 18.6 22 19 22 14.5C22 10 19 10.2 19 10.2L17.5 10.2L17.5 12.2C17.5 14.6 15.4 14.5 15.4 14.5L12 14.5C9.6 14.5 9.6 16.4 9.6 16.4L9.6 20.1C9.6 20.1 9.2 22 12.1 22ZM13.8 20.7C13.4 20.7 13 20.4 13 19.9C13 19.5 13.4 19.1 13.8 19.1C14.3 19.1 14.7 19.5 14.7 19.9C14.7 20.4 14.3 20.7 13.8 20.7Z" fill="#FFE052"/>
    </svg>
  `;

  // Leaf SVG
  const leafSvg = `
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M11 20A7 7 0 0 1 9.8 6.1C15.5 5 17 4.48 19 2c1 2 2 4.18 2 8 0 5.5-4.78 10-10 10Z"></path>
      <path d="M2 21c0-3 1.85-5.36 5.08-6C9.5 14.52 12 13 13 12"></path>
    </svg>
  `;

  // Terminal >_ SVG
  const termPromptSvg = `
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="4 17 10 11 4 5"></polyline>
      <line x1="12" y1="19" x2="20" y2="19"></line>
    </svg>
  `;

  container.innerHTML = `
    <div class="main-content">
      <!-- Top Navigation Bar matching screenshot -->
      <div class="top-nav-bar">
        <!-- Mascot Selector Group: [Pixel] [Byte] [Mochi] -->
        <div class="pet-selector-group">
          <button class="pet-pill-btn ${activePet === 'ghost' ? 'active' : ''}" data-pet="ghost" title="Pixel the Ghost">
            <span>👻</span>
            <span>Pixel</span>
          </button>
          <button class="pet-pill-btn ${activePet === 'cat' ? 'active' : ''}" data-pet="cat" title="Byte the Cyber Cat">
            <span>🐱</span>
            <span>Byte</span>
          </button>
          <button class="pet-pill-btn ${activePet === 'bunny' ? 'active' : ''}" data-pet="bunny" title="Mochi the Fluffy Bunny">
            <span>🐰</span>
            <span>Mochi</span>
          </button>
        </div>

        <div style="flex: 1;"></div>

        <!-- Mode Toggle Group: [Chill] [Hacker] -->
        <div class="mode-toggle-group">
          <button id="toggle-chill" class="mode-btn ${!isHacker ? 'active' : ''}">
            ${leafSvg}
            <span>Chill</span>
          </button>
          <button id="toggle-hacker" class="mode-btn ${isHacker ? 'active' : ''}">
            ${termPromptSvg}
            <span>Hacker</span>
          </button>
        </div>
      </div>

      <!-- Hero Mascot & Speech Bubble Section with previous animated pet restored -->
      <div class="mascot-hero-section">
        <div id="hero-mascot" class="mascot-container" title="Click to interact with ${activePet === 'ghost' ? 'Pixel' : activePet === 'cat' ? 'Byte' : 'Mochi'}!">
          ${createMascotSvg(activePet, 135, 'hero')}
        </div>
        <div class="speech-bubble">
          <div class="speech-bubble-tail"></div>
          <div id="speech-bubble-text" class="speech-bubble-inner-text">${state.mascotSpeech || "Hey there! I'm Setup Pal.\nI'll help you set up your development\ntools and environments safely!"}</div>
        </div>
      </div>

      <!-- Workspace Area -->
      <div class="workspace-scroll-area">
        ${isHacker && state.terminalTab === 'terminal' ? `
          <!-- Hacker Terminal View -->
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
                <span style="color: var(--accent);">> </span>
                <span class="terminal-cursor"></span>
              </div>
            </div>
          </div>
        ` : `
          ${(state.chatMessages && state.chatMessages.length > 0) ? `
            <!-- Chat Thread Stream -->
            <div class="chat-thread-stream" id="chat-thread-stream">
              ${messages.map(m => renderMessage(m, {
                isLast: m === lastMsg,
                isNew: !firstRender && newMsgIds.includes(m.id),
                activePet,
                ui: chatUi,
              })).join('')}
            </div>
          ` : `
            <!-- Main Ask Card matching screenshot -->
            <div class="ask-card">
              <div class="ask-card-header">
                <div class="terminal-square-icon">>_</div>
                <h2 class="ask-card-title">Ask anything in the chat window</h2>
              </div>

              <p class="ask-card-desc">
                Tell me what you'd like to set up. I'll inspect your system, ask clarifying questions, and formulate a verified setup plan.
              </p>

              <!-- 3 Starter Cards in 3-column row -->
              <div class="starter-cards-grid">
                <!-- Card 1: Minecraft & OptiFine -->
                <div class="starter-card" data-prompt="I want to download minecraft mods like optifine.">
                  <div class="starter-card-top">
                    <div class="terminal-square-icon-sm">>_</div>
                    <div class="starter-card-title">Minecraft & OptiFine</div>
                    <div class="starter-card-arrow">&rarr;</div>
                  </div>
                  <div class="starter-card-body">
                    Install and configure Minecraft with OptiFine, mods, and performance tweaks.
                  </div>
                </div>

                <!-- Card 2: Python AI & Data Science -->
                <div class="starter-card" data-prompt="setup Python AI data science venv">
                  <div class="starter-card-top">
                    <div class="starter-card-icon-wrap">${pythonLogoSvg}</div>
                    <div class="starter-card-title">Python AI &<br/>Data Science</div>
                    <div class="starter-card-arrow">&rarr;</div>
                  </div>
                  <div class="starter-card-body">
                    Set up Python, virtualenvs, PyTorch, Jupyter, and common data science tools.
                  </div>
                </div>

                <!-- Card 3: C++20 Dev Toolchain -->
                <div class="starter-card" data-prompt="set me up for C++20 dev">
                  <div class="starter-card-top">
                    <div class="terminal-square-icon-sm">>_</div>
                    <div class="starter-card-title">C++20 Dev Toolchain</div>
                    <div class="starter-card-arrow">&rarr;</div>
                  </div>
                  <div class="starter-card-body">
                    Install compilers, CMake, and a modern C++ development environment.
                  </div>
                </div>
              </div>

              <!-- Bottom Indicator: Dry-Run Simulation Active -->
              <div class="dry-run-pill-row">
                <div class="dry-run-pill">
                  <span class="dry-run-green-dot"></span>
                  <span>Dry-Run Simulation Active</span>
                  <span class="dry-run-dot-sep">•</span>
                  <span>No random downloads will ever alter your machine</span>
                </div>
              </div>
            </div>
          `}

          <!-- Chill Plan Card (when plan exists) -->
          ${(plan && plan.tasks && plan.tasks.length > 0) ? `
          <div class="plan-card" style="margin-top: 16px;">
            <div class="plan-title">${plan.title}</div>
            <div class="plan-subtitle">${plan.subtitle}</div>

            <div class="plan-steps-list">
              ${plan.tasks.map(t => `
                <div class="step-row" data-task-id="${t.id}">
                  <div class="step-left">
                    <div class="step-number">${t.num}</div>
                    <div class="step-info">
                      <div class="step-title">${t.title}</div>
                      <div class="step-desc">${t.desc}</div>
                      <div class="step-meta-row" style="margin-top: 5px; display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
                        <span class="step-source-pill" style="font-size: 11px; padding: 2px 7px; background: rgba(0,0,0,0.06); border-radius: 4px;">
                          ${t.sourceUrl ? `<a href="${t.sourceUrl}" target="_blank" rel="noopener noreferrer" style="color: inherit; text-decoration: underline;" title="Direct publisher link">🌐 From <strong>${t.sourceDomain || t.source}</strong></a>` : `<span>📦 ${t.sourceDomain || t.source}</span>`}
                        </span>
                        <span class="step-verdict-text" style="font-size: 11px; color: ${t.verdict === 'GREEN' ? '#2e7d32' : t.verdict === 'YELLOW' ? '#b26a00' : '#c62828'};">
                          <strong>${t.verdictTitle || t.verdict}</strong>: ${t.verdictDesc || ''}
                        </span>
                      </div>
                    </div>
                  </div>
                  <div class="badge ${t.verdict === 'GREEN' ? 'badge-green' : t.verdict === 'YELLOW' ? 'badge-yellow' : 'badge-red'}">
                    ${t.verdict}
                  </div>
                </div>
              `).join('')}
            </div>

            <div class="plan-trust-row" style="margin: 14px 0 6px; padding: 8px 12px; background: rgba(0,0,0,0.04); border-radius: 6px; display: flex; align-items: center; gap: 8px; font-size: 12px;">
              <input type="checkbox" id="trust-winget-checkbox" style="cursor: pointer;" />
              <label for="trust-winget-checkbox" style="cursor: pointer; color: var(--ink-soft);">
                Don't ask me about verified winget packages again (narrow trust)
              </label>
            </div>

            <div class="plan-actions-row">
              <button id="btn-approve-plan" class="btn-approve">Approve safe steps</button>
              <button id="btn-review-yellow" class="btn-review">Review warnings</button>
            </div>
          </div>
          ` : ''}
        `}
      </div>

      <!-- Bottom Chat Composer Area matching screenshot -->
      <div class="bottom-bar">
        <div class="composer-box">
          <span class="composer-paperclip" title="Attach file">📎</span>
          <span class="composer-slash">/</span>
          <input id="chat-input" class="composer-input" type="text" placeholder="Tell me what to set up..." />
          <button id="btn-chat-send" class="btn-send">
            <span>Send</span>
            <span>&rarr;</span>
          </button>
        </div>

        <!-- Suggestion Chips matching screenshot -->
        <div class="chips-row">
          <button class="suggestion-chip" data-chip="minecraft">Minecraft + mods</button>
          <button class="suggestion-chip" data-chip="cpp">C++ toolchain</button>
          <button class="suggestion-chip" data-chip="python">Python data science</button>
          <button class="suggestion-chip chip-attack" data-chip="attack">
            <span class="chip-warn-icon">⚠️</span>
            <span>Simulate Attack Radar</span>
          </button>
        </div>
      </div>
    </div>
  `;

  // Sync mascot state
  const mascotSvg = container.querySelector('#hero-ghost-root');
  if (mascotSvg) {
    mascotSvg.dataset.state = state.mascotState;
    mascotSvg.dataset.mode = state.mode;
  }

  // Pet Selector Buttons
  container.querySelectorAll('.pet-selector-group button').forEach(btn => {
    btn.onclick = () => {
      const pet = btn.getAttribute('data-pet');
      store.setActivePet(pet);
      petVoiceEngine.trigger('pet_change');
    };
  });

  // Event Listeners for Mode Toggle
  container.querySelector('#toggle-chill').onclick = () => {
    store.setMode('chill');
    petVoiceEngine.trigger('mode_toggle', { mode: 'chill' });
  };
  container.querySelector('#toggle-hacker').onclick = () => {
    store.setMode('hacker');
    petVoiceEngine.trigger('mode_toggle', { mode: 'hacker' });
  };

  // Mascot Click -> Poke event triggers pet voiceline and poke animation
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

  // Plan Approval -> triggers execution
  const btnApprove = container.querySelector('#btn-approve-plan');
  if (btnApprove) {
    btnApprove.onclick = () => {
      const trustWinget = container.querySelector('#trust-winget-checkbox')?.checked;
      const categories = trustWinget ? ['verified_winget'] : [];
      petVoiceEngine.trigger('plan_approve');
      bridge.approveAndRunPlan(categories);
    };
  }

  // Review Yellow button
  const btnReview = container.querySelector('#btn-review-yellow');
  if (btnReview) {
    btnReview.onclick = () => {
      const heldTasks = (plan.tasks || []).filter(t => t.verdict === 'YELLOW');
      if (heldTasks.length > 0) {
        const firstHeld = heldTasks[0];
        petVoiceEngine.trigger('warning_held', { title: firstHeld.title });
        const reasons = (firstHeld.reasons && firstHeld.reasons.length > 0)
          ? firstHeld.reasons.join('\n- ')
          : 'Brand new package, young release, zero community endorsements.';
        alert(`HELD TASK: ${firstHeld.title}\n- Source: ${firstHeld.source}\n- Check: ${firstHeld.desc}\n- Zero-Trust Policy:\n- ${reasons}`);
      } else {
        alert("All steps in the current plan are verified GREEN!");
      }
    };
  }

  // Composer Send & Input handling with listening animation
  const chatInput = container.querySelector('#chat-input');
  const btnSend = container.querySelector('#btn-chat-send');

  if (chatInput) {
    // Restore preserved input text and cursor position
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
      const mascotSvg = container.querySelector('#hero-ghost-root');
      if (mascotSvg) {
        mascotSvg.dataset.state = 'listening';
      }
      clearTimeout(typingTimer);
      typingTimer = setTimeout(() => {
        if (mascotSvg && mascotSvg.dataset.state === 'listening') {
          mascotSvg.dataset.state = store.getState().mascotState || 'idle';
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

  // Starter Cards (Interactive prompt cards)
  container.querySelectorAll('.starter-card').forEach(card => {
    card.onclick = () => {
      const prompt = card.getAttribute('data-prompt');
      petVoiceEngine.trigger('prompt_selected', { prompt });
      bridge.submitGoal(prompt);
    };
  });

  // Chat Option Pills (Quick response buttons inside assistant question messages)
  container.querySelectorAll('.chat-option-pill').forEach(btn => {
    btn.onclick = () => {
      const choice = btn.getAttribute('data-choice');
      sounds.play('ui.click');
      if (choice === 'open_settings') {
        store.setActivePage('settings');
        return;
      }
      bridge.submitGoal(choice);
    };
  });

  // Choice groups: remember the selection across re-renders and keep the summary in sync
  container.querySelectorAll('.choice-input').forEach(input => {
    input.onchange = () => {
      chatUi.selections[`${input.dataset.form}:${input.dataset.gi}`] = Number(input.dataset.oi);
      sounds.play('ui.click');
      updateChoiceSummary(container, input.dataset.form);
    };
  });

  // Continue: send every answer in one message
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

  // Collapsible system check: remember which ones the user closed
  container.querySelectorAll('details.chat-sys').forEach(details => {
    details.addEventListener('toggle', () => {
      const key = details.getAttribute('data-key');
      if (details.open) chatUi.collapsed.delete(key);
      else chatUi.collapsed.add(key);
    });
  });

  // Copy path
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

  // Code Block Copy to Clipboard
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

  // Suggestion Chips
  container.querySelectorAll('.suggestion-chip').forEach(chip => {
    chip.onclick = () => {
      const type = chip.getAttribute('data-chip');
      sounds.play('ui.click');
      if (type === 'attack') {
        bridge.runAttackRadar();
      } else if (type === 'minecraft') {
        const goal = 'I want to download minecraft mods like optifine.';
        petVoiceEngine.trigger('prompt_selected', { prompt: goal });
        bridge.submitGoal(goal);
      } else if (type === 'cpp') {
        const goal = 'set me up for C++20 dev';
        petVoiceEngine.trigger('prompt_selected', { prompt: goal });
        bridge.submitGoal(goal);
      } else if (type === 'python') {
        const goal = 'setup Python AI data science venv';
        petVoiceEngine.trigger('prompt_selected', { prompt: goal });
        bridge.submitGoal(goal);
      }
    };
  });

  // Scrolling: keep the reader's place, and land on the START of a new long reply
  // (jumping to the bottom would hide the beginning of what the assistant just said).
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
