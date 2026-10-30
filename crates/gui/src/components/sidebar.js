/**
 * SetupPal: Sidebar Component
 * Main nav, recent chats with a status icon each, and Settings + the safety level in the footer.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { SAFETY_LEVELS } from '../state/prefs.js';
import { esc } from './html.js';
import { icon } from './icon.js';

const NAV = [
  { page: 'chats', label: 'Chats', iconName: 'message' },
  { page: 'history', label: 'History', iconName: 'history' },
  { page: 'downloads', label: 'Downloads', iconName: 'download' },
  { page: 'accounts', label: 'Accounts', iconName: 'user' },
];

// status -> [icon, screen-reader label]
const RECENT_STATUS = {
  replying: ['refresh', 'Replying'],
  running: ['refresh', 'Running'],
  blocked: ['shieldX', 'Blocked step'],
  held: ['alert', 'Waiting for review'],
  failed: ['alert', 'Last reply failed'],
  done: ['check', 'Finished'],
  planned: ['clipboard', 'Plan ready'],
  chat: ['message', 'Chat'],
};

/** Derived from what the conversation actually holds; nothing is guessed. */
function conversationStatus(conv, isActive, state) {
  const plan = isActive ? state.activePlan : conv.activePlan;
  const messages = isActive ? state.chatMessages : conv.chatMessages;
  if (isActive && state.isChatGenerating) return 'replying';
  const tasks = plan?.tasks || [];
  if (tasks.some(t => t.status === 'running')) return 'running';
  if (tasks.some(t => t.status === 'blocked')) return 'blocked';
  if (tasks.some(t => t.status === 'held')) return 'held';
  if (plan?.status === 'completed' || (tasks.length && tasks.every(t => t.status === 'done'))) return 'done';
  const last = messages?.[messages.length - 1];
  if (last?.failed) return 'failed';
  if (tasks.length) return 'planned';
  return 'chat';
}

export function renderSidebar(container) {
  const state = store.getState();
  const safety = state.settings?.safetyLevel || 'strict';
  const safetyLabel = SAFETY_LEVELS[safety]?.label || safety;
  const convById = new Map((state.conversations || []).map(c => [c.id, c]));

  container.innerHTML = `
    <div class="sidebar">
      <div class="sidebar-header">
        <div class="logo-row">
          <div class="logo-mascot" aria-hidden="true">${icon('ghost', 28)}</div>
          <div class="logo-text">Setup Pal</div>
        </div>
        <button id="btn-new-chat" class="btn-new-setup" type="button">
          ${icon('plus', 16)}
          <span>New setup</span>
        </button>
      </div>

      <nav class="sidebar-nav" aria-label="Main">
        ${NAV.map(item => {
          const current = state.activePage === item.page;
          return `
            <button type="button" id="nav-${item.page}" class="nav-item ${current ? 'active' : ''}" data-page="${item.page}" ${current ? 'aria-current="page"' : ''}>
              ${icon(item.iconName, 16, 'nav-icon')}
              <span>${item.label}</span>
            </button>
          `;
        }).join('')}
      </nav>

      <div class="recent-header-row">
        <h2 class="section-label" id="recent-heading">Recent</h2>
        <div class="recent-divider-line"></div>
      </div>

      <ul class="recent-list" aria-labelledby="recent-heading">
        ${state.recentChats.map(c => {
          const status = conversationStatus(convById.get(c.id) || {}, c.active, state);
          const [statusIcon, statusLabel] = RECENT_STATUS[status];
          const title = esc(c.title);
          return `
            <li class="recent-item ${c.active ? 'selected' : ''}">
              <button type="button" id="recent-${esc(c.id)}" class="recent-item-title-wrap" data-chat-id="${esc(c.id)}" ${c.active ? 'aria-current="true"' : ''}>
                <span class="recent-status-icon status-${status}" aria-hidden="true">${icon(statusIcon, 14)}</span>
                <span class="sr-only">${statusLabel}:</span>
                <span class="recent-title-text">${title}</span>
              </button>
              ${state.recentChats.length > 1 ? `
                <button type="button" class="btn-delete-recent" data-chat-id="${esc(c.id)}" title="Delete chat" aria-label="Delete ${title}">${icon('close', 14)}</button>
              ` : ''}
            </li>
          `;
        }).join('')}
      </ul>

      <div class="sidebar-bottom">
        <button type="button" id="btn-open-settings" class="btn-sidebar-bottom ${state.activePage === 'settings' ? 'active' : ''}" ${state.activePage === 'settings' ? 'aria-current="page"' : ''}>
          ${icon('settings', 15)}
          <span>Settings</span>
        </button>
        <button type="button" id="safety-chip-btn" class="safety-dot safety-${esc(safety)}" title="Safety level: ${esc(safetyLabel)}. Change it in Settings." aria-label="Safety level ${esc(safetyLabel)}. Open safety settings">
          <span class="safety-dot-orb" aria-hidden="true"></span>
          <span class="safety-dot-label">${esc(safetyLabel)}</span>
        </button>
      </div>
    </div>
  `;

  container.querySelector('#btn-new-chat').onclick = () => store.createConversation();

  container.querySelectorAll('.nav-item').forEach(btn => {
    btn.onclick = () => store.setActivePage(btn.getAttribute('data-page'));
  });

  container.querySelectorAll('.recent-item-title-wrap').forEach(item => {
    item.onclick = () => store.switchConversation(item.getAttribute('data-chat-id'));
  });

  container.querySelectorAll('.btn-delete-recent').forEach(btn => {
    btn.onclick = (e) => {
      e.stopPropagation();
      store.closeConversation(btn.getAttribute('data-chat-id'));
    };
  });

  container.querySelector('#safety-chip-btn').onclick = () => {
    sounds.play('ui.click');
    store.setActivePage('settings');
    requestAnimationFrame(() => {
      const section = document.getElementById('settings-safety');
      section?.scrollIntoView({ block: 'start' });
      document.getElementById('setting-safety')?.focus({ preventScroll: true });
    });
  };

  container.querySelector('#btn-open-settings').onclick = () => store.setActivePage('settings');
}
