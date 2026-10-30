/**
 * SetupPal: Application Entrypoint
 */

import './styles/tokens.css';
import './styles/base.css';
import './styles/ghost.css';
import './styles/components.css';
import './styles/chat.css';

import { store } from './state/store.js';
import { simulation } from './state/simulation.js';
import { bridge } from './api/bridge.js';
import { renderSidebar } from './components/sidebar.js';
import { renderChatView } from './components/chat-view.js';
import { updateStreamingMessage } from './components/chat-message.js';
import { renderTaskPanel } from './components/task-panel.js';
import { renderDownloadsView } from './components/downloads-view.js';
import { renderHistoryView } from './components/history-view.js';
import { renderAccountsView } from './components/accounts-view.js';
import { renderSettingsView } from './components/settings-view.js';
import { renderAnimationLab } from './components/animation-lab.js';
import { isModalOpen } from './components/modal.js';

const VIEWS = {
  chats: renderChatView,
  downloads: renderDownloadsView,
  history: renderHistoryView,
  accounts: renderAccountsView,
  settings: renderSettingsView,
};

/** Views rebuild their DOM on every update; keep keyboard focus (and caret) on the same control. */
function captureFocus() {
  const el = document.activeElement;
  if (!el || el === document.body || !el.id) return null;
  const hasSelection = typeof el.selectionStart === 'number';
  return {
    id: el.id,
    start: hasSelection ? el.selectionStart : null,
    end: hasSelection ? el.selectionEnd : null,
  };
}

function restoreFocus(saved) {
  if (!saved) return;
  const el = document.getElementById(saved.id);
  if (!el || el === document.activeElement) return;
  el.focus({ preventScroll: true });
  if (saved.start !== null && typeof el.setSelectionRange === 'function') {
    try { el.setSelectionRange(saved.start, saved.end); } catch { /* input types without selection */ }
  }
}

function initApp() {
  const appContainer = document.getElementById('app');
  if (!appContainer) return;

  document.addEventListener('visibilitychange', () => {
    document.documentElement.classList.toggle('document-hidden', document.hidden);
  });

  appContainer.innerHTML = `
    <aside id="sidebar-container" class="sidebar-host"></aside>
    <main id="main-view-container" class="main-view"></main>
    <aside id="task-panel-container" class="task-panel-host" aria-label="Tasks" hidden></aside>
    <div id="modal-container"></div>
  `;

  const sidebarEl = document.getElementById('sidebar-container');
  const mainViewEl = document.getElementById('main-view-container');
  const taskPanelEl = document.getElementById('task-panel-container');
  const modalEl = document.getElementById('modal-container');

  window.addEventListener('setuppal:stream-delta', (event) => {
    const { conversationId, messageId, text } = event.detail || {};
    if (store.getState().activeConversationId !== conversationId) return;
    updateStreamingMessage(messageId, text);
  });

  function render() {
    const state = store.getState();
    const focus = captureFocus();

    renderSidebar(sidebarEl);
    (VIEWS[state.activePage] || renderChatView)(mainViewEl);

    // Task panel only exists while the chat has a plan with steps
    const hasPlanSteps = Boolean(state.activePlan?.tasks?.length);
    if (state.activePage === 'chats' && hasPlanSteps) {
      taskPanelEl.hidden = false;
      taskPanelEl.classList.toggle('is-collapsed', !state.tasksOpen);
      renderTaskPanel(taskPanelEl);
    } else {
      taskPanelEl.hidden = true;
      taskPanelEl.innerHTML = '';
    }

    renderAnimationLab(modalEl);
    restoreFocus(focus);
  }

  store.subscribe(render);
  render();

  // Esc stops whatever is running: a streaming reply first, then a plan run. Dialogs handle their own Esc.
  window.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape' || e.defaultPrevented || isModalOpen()) return;
    if (store.getState().isChatGenerating) bridge.stopChat();
    else simulation.stopRun();
  });

  bridge.init();
}

window.addEventListener('DOMContentLoaded', initApp);
