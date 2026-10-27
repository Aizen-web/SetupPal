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
          <div class="recent-item ${c.active ? 'selected' : ''}" data-chat-id="${c.id}" style="display: flex; align-items: center; justify-content: space-between; gap: 4px;">
            <div class="recent-item-title-wrap" data-chat-id="${c.id}" style="display: flex; align-items: center; gap: 8px; flex: 1; overflow: hidden; cursor: pointer;">
              <span class="recent-icon">${c.icon || '💬'}</span>
              <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1;">${c.title}</span>
            </div>
            ${state.recentChats.length > 1 ? `
              <button class="btn-delete-recent" data-chat-id="${c.id}" title="Delete setup" style="background: none; border: none; color: var(--ink-soft); cursor: pointer; padding: 2px 5px; font-size: 13px; opacity: 0.5; border-radius: 4px; line-height: 1;" onmouseover="this.style.opacity='1'; this.style.color='var(--v-red)'" onmouseout="this.style.opacity='0.5'; this.style.color='var(--ink-soft)'">×</button>
            ` : ''}
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
    store.createConversation();
  };

  container.querySelectorAll('.nav-item').forEach(btn => {
    btn.onclick = () => {
      const page = btn.getAttribute('data-page');
      store.setActivePage(page);
    };
  });

  container.querySelectorAll('.recent-item-title-wrap').forEach(item => {
    item.onclick = (e) => {
      e.stopPropagation();
      const chatId = item.getAttribute('data-chat-id');
      store.switchConversation(chatId);
    };
  });

  container.querySelectorAll('.btn-delete-recent').forEach(btn => {
    btn.onclick = (e) => {
      e.stopPropagation();
      const chatId = btn.getAttribute('data-chat-id');
      store.closeConversation(chatId);
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
