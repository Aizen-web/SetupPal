/**
 * SetupPal: Chat & Execution Workspace View
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { simulation } from '../state/simulation.js';
import { createMascotSvg } from '../mascot/pets.js';

export function renderChatView(container) {
  const state = store.getState();
  const plan = state.activePlan;
  const isHacker = state.mode === 'hacker';
  const activePet = state.activePet || 'ghost';

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
          <!-- Chill Plan Card -->
          <div class="plan-card">
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
                    </div>
                  </div>
                  <div class="badge ${t.verdict === 'GREEN' ? 'badge-green' : t.verdict === 'YELLOW' ? 'badge-yellow' : 'badge-red'}">
                    ${t.verdict}
                  </div>
                </div>
              `).join('')}
            </div>

            <div class="plan-actions-row">
              <button id="btn-approve-plan" class="btn-approve">Approve 4 safe steps</button>
              <button id="btn-review-yellow" class="btn-review">Review the yellow one</button>
            </div>
          </div>
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

  // Pet Selector Buttons
  container.querySelectorAll('.pet-selector-group button').forEach(btn => {
    btn.onclick = () => {
      const pet = btn.getAttribute('data-pet');
      store.setActivePet(pet);
    };
  });

  // Event Listeners
  container.querySelector('#toggle-chill').onclick = () => {
    store.setMode('chill');
  };
  container.querySelector('#toggle-hacker').onclick = () => {
    store.setMode('hacker');
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

  // Mascot Click -> Celebrates with pet personality!
  container.querySelector('#hero-mascot').onclick = () => {
    sounds.play('task.done');
    let msg = "Yay! I am Pixel, your zero-trust guardian ghost!";
    if (activePet === 'cat') msg = "Purrrr! Byte the cat is watching over every single hash!";
    if (activePet === 'bunny') msg = "Binky hop! Mochi loves clean, safe package setups!";
    store.setMascotState('celebrate', msg);
    setTimeout(() => {
      store.setMascotState('idle');
    }, 2200);
  };

  // Plan Approval -> triggers Hacker Mode & real-time simulation!
  const btnApprove = container.querySelector('#btn-approve-plan');
  if (btnApprove) {
    btnApprove.onclick = () => {
      simulation.approveAndRunPlan();
    };
  }

  // Review Yellow button
  const btnReview = container.querySelector('#btn-review-yellow');
  if (btnReview) {
    btnReview.onclick = () => {
      sounds.play('verdict.yellow');
      store.setMascotState('yellow', "FastLight Pro has zero endorsements and was uploaded 2 hours ago. We kept it held for your safety.");
      alert("HELD TASK: FastLight Pro\n- Modrinth project ID: fastlight-pro\n- Author: unknown_dev99\n- Reason: Brand new package, zero community endorsements, no publisher hash.\n- Recommendation: Keep in quarantine until audited.");
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
    let typingTimer = null;
    chatInput.oninput = () => {
      if (chatInput.value.length > 0 && state.mascotState === 'idle') {
        store.setMascotState('listening');
      }
      clearTimeout(typingTimer);
      typingTimer = setTimeout(() => {
        if (store.getState().mascotState === 'listening') {
          store.setMascotState('idle');
        }
      }, 1500);
    };
  }

  const handleSend = () => {
    const text = chatInput.value.trim();
    if (!text) return;
    chatInput.value = '';
    sounds.play('msg.send');

    if (text.toLowerCase() === '/hack') {
      store.setMode('hacker');
      return;
    }
    if (text.toLowerCase() === '/chill') {
      store.setMode('chill');
      return;
    }

    store.setMascotState('thinking', `Analyzing "${text}" against verified package policies...`);
    setTimeout(() => {
      sounds.play('msg.receive');
      store.setMascotState('speaking', `I drafted a safe zero-trust plan for "${text}". Review the steps!`);
    }, 1200);
  };

  if (btnSend && chatInput) {
    btnSend.onclick = handleSend;
    chatInput.onkeydown = (e) => {
      if (e.key === 'Enter') handleSend();
    };
  }

  // Suggestion Chips
  container.querySelectorAll('.suggestion-chip').forEach(chip => {
    chip.onclick = () => {
      const type = chip.getAttribute('data-chip');
      sounds.play('ui.click');
      if (type === 'attack') {
        simulation.simulateAttackRadar();
      } else if (type === 'minecraft') {
        store.setMascotState('idle', "Here's my plan. Take a look before I touch anything.");
      } else if (type === 'cpp') {
        store.setMascotState('thinking', "C++20 environment plan: MSVC build tools + Clang + CMake. Strict hash verification active.");
      } else if (type === 'python') {
        store.setMascotState('speaking', "Python AI plan: Isolated virtualenv with verified PyPI wheels (NumPy, PyTorch, Pandas).");
      }
    };
  });
}
