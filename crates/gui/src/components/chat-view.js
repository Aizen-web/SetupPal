/**
 * SetupPal: Chat & Execution Workspace View
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { simulation } from '../state/simulation.js';
import { bridge } from '../api/bridge.js';
import { createMascotSvg } from '../mascot/pets.js';
import { petVoiceEngine } from '../mascot/voicelines.js';

export function renderChatView(container) {
  const state = store.getState();
  const plan = state.activePlan;
  const isHacker = state.mode === 'hacker';
  const activePet = state.activePet || 'ghost';

  // Preserve chat input state if user was typing
  const prevInput = container.querySelector('#chat-input');
  const hadFocus = prevInput && document.activeElement === prevInput;
  const prevValue = prevInput ? prevInput.value : '';
  const selStart = prevInput ? prevInput.selectionStart : 0;
  const selEnd = prevInput ? prevInput.selectionEnd : 0;

  container.innerHTML = `
    <div class="main-content">
      <!-- Top Navigation Bar -->
      <div class="top-nav-bar">
        <div class="hacker-tab-strip">
          <button class="hacker-tab-btn ${state.terminalTab === 'terminal' ? 'active' : ''}" id="tab-terminal">Terminal</button>
          <button class="hacker-tab-btn ${state.terminalTab === 'chat' ? 'active' : ''}" id="tab-chat">Chat</button>
        </div>

        <!-- Pet Selector Pill -->
        <div class="pet-selector-group" style="display: flex; align-items: center; gap: 4px; background-color: var(--bg-surface); padding: 3px; border-radius: var(--radius-pill); border: 1px solid var(--border);">
          <button class="mode-btn ${activePet === 'ghost' ? 'active' : ''}" data-pet="ghost" title="Pixel the Ghost">👻 Pixel</button>
          <button class="mode-btn ${activePet === 'cat' ? 'active' : ''}" data-pet="cat" title="Byte the Cyber Cat">🐱 Byte</button>
          <button class="mode-btn ${activePet === 'bunny' ? 'active' : ''}" data-pet="bunny" title="Mochi the Fluffy Bunny">🐰 Mochi</button>
        </div>

        <div style="flex: 1;"></div>

        <!-- Mode Toggle Pill -->
        <div class="mode-toggle-group">
          <button id="toggle-chill" class="mode-btn ${!isHacker ? 'active' : ''}">Chill</button>
          <button id="toggle-hacker" class="mode-btn ${isHacker ? 'active' : ''}">Hacker</button>
        </div>
      </div>

      <!-- Conversation Tab Strip -->
      <div class="conv-tabs-strip" style="display: flex; align-items: center; gap: 6px; padding: 6px 18px; background: var(--bg-surface); border-bottom: 1px solid var(--border); overflow-x: auto;">
        ${(state.conversations || []).map(c => `
          <div class="conv-tab-pill ${c.id === state.activeConversationId ? 'active' : ''}" data-conv-id="${c.id}" style="display: inline-flex; align-items: center; gap: 7px; padding: 5px 12px; border-radius: var(--radius-pill); font-size: 12px; font-weight: 500; cursor: pointer; background: ${c.id === state.activeConversationId ? 'var(--btn-primary)' : 'var(--bg-canvas)'}; color: ${c.id === state.activeConversationId ? 'var(--btn-primary-text)' : 'var(--ink-soft)'}; border: 1px solid ${c.id === state.activeConversationId ? 'transparent' : 'var(--border)'}; transition: all 0.18s ease; white-space: nowrap; box-shadow: ${c.id === state.activeConversationId ? '0 1px 4px rgba(0,0,0,0.08)' : 'none'};">
            <span>${c.icon || '💬'}</span>
            <span style="max-width: 130px; overflow: hidden; text-overflow: ellipsis;">${c.title}</span>
            ${(state.conversations || []).length > 1 ? `
              <span class="conv-tab-close" data-conv-id="${c.id}" title="Close setup" style="font-size: 13px; line-height: 1; padding: 1px 4px; border-radius: 4px; opacity: 0.65; cursor: pointer; transition: all 0.15s ease;" onmouseover="this.style.opacity='1'; this.style.backgroundColor='rgba(0,0,0,0.15)'" onmouseout="this.style.opacity='0.65'; this.style.backgroundColor='transparent'">×</span>
            ` : ''}
          </div>
        `).join('')}
        <button id="btn-tab-new-setup" title="New Setup Tab" style="display: inline-flex; align-items: center; justify-content: center; width: 26px; height: 26px; border-radius: var(--radius-pill); background: var(--bg-canvas); border: 1px dashed var(--border); color: var(--ink); cursor: pointer; font-size: 14px; font-weight: 600; transition: all 0.15s ease;" onmouseover="this.style.borderColor='var(--accent)'; this.style.transform='scale(1.06)'" onmouseout="this.style.borderColor='var(--border)'; this.style.transform='scale(1)'">
          +
        </button>
      </div>

      <!-- Hero Mascot Section -->
      <div class="mascot-hero-section">
        <div id="hero-mascot" class="mascot-container" title="Click to interact!">
          ${createMascotSvg(activePet, 130, 'hero')}
        </div>
        <div class="speech-bubble">
          <span id="speech-bubble-text">${state.mascotSpeech}</span>
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
            <div class="chat-thread-stream" style="display: flex; flex-direction: column; gap: 14px; margin-bottom: 20px;">
              ${state.chatMessages.map(m => `
                <div class="chat-bubble ${m.sender === 'user' ? 'user-bubble' : 'bot-bubble'}" style="align-self: ${m.sender === 'user' ? 'flex-end' : 'flex-start'}; max-width: 88%; padding: 13px 18px; border-radius: 12px; background: ${m.sender === 'user' ? 'var(--btn-primary)' : 'var(--bg-surface)'}; color: ${m.sender === 'user' ? 'var(--btn-primary-text)' : 'var(--ink)'}; border: 1px solid var(--border); box-shadow: 0 1px 3px rgba(0,0,0,0.03);">
                  <div style="font-size: 13.5px; line-height: 1.6; white-space: pre-wrap;">${m.text}</div>
                  ${m.options && m.options.length > 0 ? `
                    <div class="chat-options-group" style="display: flex; gap: 8px; flex-wrap: wrap; margin-top: 12px; padding-top: 10px; border-top: 1px dashed var(--border-subtle);">
                      ${m.options.map(opt => `
                        <button class="chat-option-pill" data-choice="${opt.value}" style="background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 12px; border-radius: var(--radius-pill); font-size: 12px; font-weight: 500; cursor: pointer; transition: all 0.2s ease; display: inline-flex; align-items: center; gap: 4px;" onmouseover="this.style.borderColor='var(--accent)'; this.style.transform='translateY(-1px)'" onmouseout="this.style.borderColor='var(--border)'; this.style.transform='none'">
                          ${opt.label}
                        </button>
                      `).join('')}
                    </div>
                  ` : ''}
                  <div style="font-size: 10px; margin-top: 6px; text-align: right; opacity: 0.65;">${m.time}</div>
                </div>
              `).join('')}
            </div>
          ` : `
            <!-- Modern Empty State: Ask anything in the chat window -->
            <div class="empty-chat-container" style="display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; padding: 40px 24px 32px; margin: 12px 0 24px; background: radial-gradient(circle at 50% 20%, rgba(139, 92, 246, 0.08) 0%, rgba(59, 130, 246, 0.03) 60%, transparent 100%); border: 1px dashed var(--border); border-radius: 20px; transition: all 0.3s ease;">
              
              <div style="display: inline-flex; align-items: center; gap: 8px; background: rgba(139, 92, 246, 0.1); border: 1px solid rgba(139, 92, 246, 0.25); color: var(--accent); padding: 5px 14px; border-radius: var(--radius-pill); font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; margin-bottom: 16px;">
                <span>✨ Zero-Trust Assistant Ready</span>
              </div>

              <h2 style="font-size: 28px; font-weight: 700; letter-spacing: -0.03em; color: var(--ink); margin: 0 0 10px 0;">
                Ask anything in the chat window
              </h2>

              <p style="font-size: 14.5px; color: var(--ink-soft); max-width: 540px; line-height: 1.6; margin: 0 0 26px 0;">
                Tell me what you'd like to set up. I'll inspect your PC, ask any clarifying questions, and formulate a verified zero-trust plan.
              </p>

              <!-- Quick Starter Cards -->
              <div class="empty-prompt-grid" style="display: grid; grid-template-columns: repeat(auto-fit, minmax(230px, 1fr)); gap: 14px; width: 100%; max-width: 820px; margin-bottom: 24px;">
                <div class="empty-prompt-card" data-prompt="I want to download minecraft mods like optifine." style="background: var(--bg-surface); border: 1px solid var(--border); border-radius: 14px; padding: 16px 18px; text-align: left; cursor: pointer; transition: all 0.22s ease; box-shadow: 0 2px 8px rgba(0,0,0,0.03);">
                  <div style="font-weight: 600; font-size: 14px; color: var(--ink); margin-bottom: 5px; display: flex; align-items: center; gap: 8px;">
                    <span style="font-size: 16px;">🎮</span>
                    <span>Minecraft & OptiFine</span>
                  </div>
                  <div style="font-size: 12px; color: var(--ink-soft); line-height: 1.45;">
                    Inspects Java runtime, checks launcher folders, and crafts safe mod steps.
                  </div>
                </div>

                <div class="empty-prompt-card" data-prompt="setup Python AI data science venv" style="background: var(--bg-surface); border: 1px solid var(--border); border-radius: 14px; padding: 16px 18px; text-align: left; cursor: pointer; transition: all 0.22s ease; box-shadow: 0 2px 8px rgba(0,0,0,0.03);">
                  <div style="font-weight: 600; font-size: 14px; color: var(--ink); margin-bottom: 5px; display: flex; align-items: center; gap: 8px;">
                    <span style="font-size: 16px;">🐍</span>
                    <span>Python AI & Data Science</span>
                  </div>
                  <div style="font-size: 12px; color: var(--ink-soft); line-height: 1.45;">
                    Isolated virtualenv, PyTorch/CUDA checks, and clean pinned hashes.
                  </div>
                </div>

                <div class="empty-prompt-card" data-prompt="set me up for C++20 dev" style="background: var(--bg-surface); border: 1px solid var(--border); border-radius: 14px; padding: 16px 18px; text-align: left; cursor: pointer; transition: all 0.22s ease; box-shadow: 0 2px 8px rgba(0,0,0,0.03);">
                  <div style="font-weight: 600; font-size: 14px; color: var(--ink); margin-bottom: 5px; display: flex; align-items: center; gap: 8px;">
                    <span style="font-size: 16px;">⚡</span>
                    <span>C++20 Dev Toolchain</span>
                  </div>
                  <div style="font-size: 12px; color: var(--ink-soft); line-height: 1.45;">
                    Compiler, CMake, and reproducible HKCU user environment paths.
                  </div>
                </div>
              </div>

              <!-- Security Indicator -->
              <div style="display: inline-flex; align-items: center; gap: 8px; font-size: 12px; color: var(--ink-soft); background: var(--bg-canvas); padding: 7px 16px; border-radius: var(--radius-pill); border: 1px solid var(--border);">
                <span style="display: inline-block; width: 7px; height: 7px; border-radius: 50%; background: #10B981; box-shadow: 0 0 8px #10B981;"></span>
                <span>Dry-Run Simulation Active · No random downloads will ever alter your machine</span>
              </div>
            </div>
          `}

          <!-- Chill Plan Card -->
          ${(plan && plan.tasks && plan.tasks.length > 0) ? `
          <div class="plan-card" style="margin-top: 8px;">
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

      <!-- Bottom Bar -->
      <div class="bottom-bar">
        <!-- Chill Composer -->
        <div class="composer-box">
          <span style="color: var(--ink-soft); font-size: 15px; cursor: pointer;" title="Attach file">📎</span>
          <span style="color: var(--ink-soft); font-size: 13px; font-family: var(--font-mono);">/</span>
          <input id="chat-input" class="composer-input" type="text" placeholder="Tell me what to set up..." />
          <button id="btn-chat-send" class="btn-send">Send</button>
        </div>

        <!-- Hacker Status Bar -->
        <div class="hacker-status-bar">
          <div style="display: flex; align-items: center; gap: 10px;">
            <span class="dot dot-green" style="animation: pulseDot 1s infinite alternate;"></span>
            <span>Working... press Esc to stop</span>
          </div>
          <button id="btn-hacker-stop" class="btn-stop">Stop</button>
        </div>

        <!-- Suggestion Chips -->
        <div class="chips-row">
          <div class="suggestion-chip" data-chip="minecraft">Minecraft + mods</div>
          <div class="suggestion-chip" data-chip="cpp">C++ toolchain</div>
          <div class="suggestion-chip" data-chip="python">Python data science</div>
          <div class="suggestion-chip" data-chip="attack" style="border-color: var(--v-red); color: var(--v-red);">⚠️ Simulate Attack Radar</div>
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

  // Conversation Tab Pills
  container.querySelectorAll('.conv-tab-pill').forEach(pill => {
    pill.onclick = (e) => {
      if (e.target.classList.contains('conv-tab-close')) return;
      const convId = pill.getAttribute('data-conv-id');
      store.switchConversation(convId);
    };
  });

  // Conversation Tab Close Buttons
  container.querySelectorAll('.conv-tab-close').forEach(btn => {
    btn.onclick = (e) => {
      e.stopPropagation();
      const convId = btn.getAttribute('data-conv-id');
      store.closeConversation(convId);
    };
  });

  // New Tab Button
  const btnTabNewSetup = container.querySelector('#btn-tab-new-setup');
  if (btnTabNewSetup) {
    btnTabNewSetup.onclick = () => {
      store.createConversation();
    };
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

  const tabTerminal = container.querySelector('#tab-terminal');
  if (tabTerminal) {
    tabTerminal.onclick = () => {
      sounds.play('ui.click');
      store.setState({ terminalTab: 'terminal' });
    };
  }

  const tabChat = container.querySelector('#tab-chat');
  if (tabChat) {
    tabChat.onclick = () => {
      sounds.play('ui.click');
      store.setState({ terminalTab: 'chat' });
    };
  }

  // Mascot Click -> Poke event triggers pet voiceline
  container.querySelector('#hero-mascot').onclick = () => {
    petVoiceEngine.trigger('poke');
    setTimeout(() => {
      if (store.getState().mascotState === 'celebrate') {
        store.setMascotState('idle');
      }
    }, 2400);
  };

  // Plan Approval -> triggers Hacker Mode & real backend execution!
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

  // Stop button
  const btnStop = container.querySelector('#btn-hacker-stop');
  if (btnStop) {
    btnStop.onclick = () => {
      simulation.stopRun();
    };
  }

  // Composer Send & Typing Animation (#14 LISTENING)
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
      // Directly animate mascot to listening state without tearing down DOM or losing focus
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

  // Empty Prompt Cards (Interactive cards in the empty chat state)
  container.querySelectorAll('.empty-prompt-card').forEach(card => {
    card.onmouseover = () => {
      card.style.borderColor = 'var(--accent)';
      card.style.transform = 'translateY(-2px)';
      card.style.boxShadow = '0 6px 16px rgba(0,0,0,0.06)';
    };
    card.onmouseout = () => {
      card.style.borderColor = 'var(--border)';
      card.style.transform = 'none';
      card.style.boxShadow = '0 2px 8px rgba(0,0,0,0.03)';
    };
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
}
