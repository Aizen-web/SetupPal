/**
 * SetupPal: Sidebar Component
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { simulation } from '../state/simulation.js';

export function renderSidebar(container) {
  const state = store.getState();

  container.innerHTML = `
    <div class="sidebar">
      <div class="sidebar-header">
        <div class="logo-row">
          <div style="font-size: 22px;">👻</div>
          <div class="logo-text">Setup Pal</div>
        </div>
        <button id="btn-new-chat" class="btn-new-setup">
          <span>+</span>
          <span>New setup</span>
        </button>
      </div>

      <nav class="sidebar-nav">
        <button class="nav-item ${state.activePage === 'chats' ? 'active' : ''}" data-page="chats">
          <span>💬</span>
          <span>Chats</span>
        </button>
        <button class="nav-item ${state.activePage === 'history' ? 'active' : ''}" data-page="history">
          <span>📜</span>
          <span>History</span>
        </button>
        <button class="nav-item ${state.activePage === 'downloads' ? 'active' : ''}" data-page="downloads">
          <span>📦</span>
          <span>Downloads</span>
        </button>
        <button class="nav-item ${state.activePage === 'accounts' ? 'active' : ''}" data-page="accounts">
          <span>👤</span>
          <span>Accounts</span>
        </button>
      </nav>

      <div class="section-label">RECENT</div>
      <div class="recent-list">
        ${state.recentChats.map(c => `
          <div class="recent-item ${c.active ? 'selected' : ''}" data-chat-id="${c.id}">
            <span class="recent-icon">${c.icon}</span>
            <span>${c.title}</span>
          </div>
        `).join('')}
      </div>

      <div class="sidebar-bottom">
        <div id="safety-chip-btn" class="safety-chip" title="Click to view Zero-Trust safety policy">
          <span>🛡️</span>
          <span>Safety: <strong>${state.settings.safetyLevel}</strong></span>
        </div>

        <button id="btn-open-lab" class="btn-sidebar-bottom">
          <span>✨</span>
          <span>Mascot Lab (19 Animations)</span>
        </button>

        <button id="btn-open-settings" class="btn-sidebar-bottom ${state.activePage === 'settings' ? 'active' : ''}">
          <span>⚙️</span>
          <span>Settings</span>
        </button>
      </div>
    </div>
  `;

  // Attach event handlers
  container.querySelector('#btn-new-chat').onclick = () => {
    sounds.play('ui.click');
    store.setActivePage('chats');
    store.setMascotState('wave-hello', "Boo... I mean, hi! What are we setting up today?");
  };

  container.querySelectorAll('.nav-item').forEach(btn => {
    btn.onclick = () => {
      const page = btn.getAttribute('data-page');
      store.setActivePage(page);
    };
  });

  container.querySelectorAll('.recent-item').forEach(item => {
    item.onclick = () => {
      sounds.play('ui.click');
      const chatId = item.getAttribute('data-chat-id');
      store.setState(s => ({
        ...s,
        recentChats: s.recentChats.map(c => ({ ...c, active: c.id === chatId })),
        activePage: 'chats'
      }));
    };
  });

  container.querySelector('#safety-chip-btn').onclick = () => {
    sounds.play('ui.click');
    alert("Zero-Trust Security Level: STRICT\n- All machine alterations require explicit approval\n- Typosquat distance & CVEs scanned via OSV.dev\n- Hash matching enforced for all binaries");
  };

  container.querySelector('#btn-open-lab').onclick = () => {
    store.toggleAnimationLab(true);
  };

  container.querySelector('#btn-open-settings').onclick = () => {
    store.setActivePage('settings');
  };
}
