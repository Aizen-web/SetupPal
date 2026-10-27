/**
 * SetupPal: Sidebar Component
 * Styled to match the exact aesthetic of the screenshot
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { createMascotHeadIconSvg } from '../mascot/mascot-art.js';

export function renderSidebar(container) {
  const state = store.getState();

  container.innerHTML = `
    <div class="sidebar">
      <div class="sidebar-header">
        <div class="logo-row">
          <div style="font-size: 24px; line-height: 1;">👻</div>
          <div class="logo-text">Setup Pal</div>
        </div>
        <button id="btn-new-chat" class="btn-new-setup">
          <span style="font-size: 16px; font-weight: 500;">+</span>
          <span>New Setup</span>
        </button>
      </div>

      <nav class="sidebar-nav">
        <button class="nav-item ${state.activePage === 'chats' ? 'active' : ''}" data-page="chats">
          <svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
          </svg>
          <span>Chats</span>
        </button>
        <button class="nav-item ${state.activePage === 'history' ? 'active' : ''}" data-page="history">
          <svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <polyline points="12 6 12 12 14 14"></polyline>
            <path d="M3.05 11a9 9 0 0 1 .5-2m-.5 2h4m-4 0v-4"></path>
          </svg>
          <span>History</span>
        </button>
        <button class="nav-item ${state.activePage === 'downloads' ? 'active' : ''}" data-page="downloads">
          <svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
            <polyline points="7 10 12 15 17 10"></polyline>
            <line x1="12" y1="15" x2="12" y2="3"></line>
          </svg>
          <span>Downloads</span>
        </button>
        <button class="nav-item ${state.activePage === 'accounts' ? 'active' : ''}" data-page="accounts">
          <svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
            <circle cx="12" cy="7" r="4"></circle>
          </svg>
          <span>Accounts</span>
        </button>
      </nav>

      <div class="recent-header-row">
        <span class="section-label">RECENT</span>
        <div class="recent-divider-line"></div>
      </div>

      <div class="recent-list">
        ${state.recentChats.map(c => `
          <div class="recent-item ${c.active ? 'selected' : ''}" data-chat-id="${c.id}">
            <div class="recent-item-title-wrap" data-chat-id="${c.id}">
              <svg class="recent-star-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon>
              </svg>
              <span class="recent-title-text">${c.title}</span>
            </div>
            ${state.recentChats.length > 1 ? `
              <button class="btn-delete-recent" data-chat-id="${c.id}" title="Delete setup">×</button>
            ` : ''}
          </div>
        `).join('')}
      </div>

      <div class="sidebar-bottom">
        <div id="safety-chip-btn" class="safety-chip" title="Click to view Zero-Trust safety policy">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"></path>
          </svg>
          <span>Safety:</span>
          <span class="safety-badge">${state.settings?.safetyLevel || 'strict'}</span>
        </div>

        <button id="btn-open-lab" class="btn-sidebar-bottom">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M10 2v7.31L4.69 19.34A2 2 0 0 0 6.43 22h11.14a2 2 0 0 0 1.74-2.66L14 9.31V2"></path>
            <line x1="8.5" y1="2" x2="15.5" y2="2"></line>
            <line x1="14" y1="9" x2="10" y2="9"></line>
          </svg>
          <span>Mascot Lab (19 Animations)</span>
        </button>

        <button id="btn-open-settings" class="btn-sidebar-bottom ${state.activePage === 'settings' ? 'active' : ''}">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="3"></circle>
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
          </svg>
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
