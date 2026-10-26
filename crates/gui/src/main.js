/**
 * SetupPal: Application Entrypoint
 */

import './styles/tokens.css';
import './styles/base.css';
import './styles/ghost.css';
import './styles/components.css';

import { store } from './state/store.js';
import { simulation } from './state/simulation.js';
import { renderSidebar } from './components/sidebar.js';
import { renderChatView } from './components/chat-view.js';
import { renderTaskPanel } from './components/task-panel.js';
import { renderDownloadsView } from './components/downloads-view.js';
import { renderHistoryView } from './components/history-view.js';
import { renderAccountsView } from './components/accounts-view.js';
import { renderSettingsView } from './components/settings-view.js';
import { renderAnimationLab } from './components/animation-lab.js';

function initApp() {
  const appContainer = document.getElementById('app');
  if (!appContainer) return;

  // Create layout DOM structure
  appContainer.innerHTML = `
    <div class="hacker-scanlines"></div>
    <div id="sidebar-container"></div>
    <div id="main-view-container" style="flex: 1; display: flex; height: 100vh; overflow: hidden;"></div>
    <div id="task-panel-container"></div>
    <div id="modal-container"></div>
  `;

  const sidebarEl = document.getElementById('sidebar-container');
  const mainViewEl = document.getElementById('main-view-container');
  const taskPanelEl = document.getElementById('task-panel-container');
  const modalEl = document.getElementById('modal-container');

  function render() {
    const state = store.getState();

    // 1. Sidebar
    renderSidebar(sidebarEl);

    // 2. Active Main View
    switch (state.activePage) {
      case 'chats':
        renderChatView(mainViewEl);
        break;
      case 'downloads':
        renderDownloadsView(mainViewEl);
        break;
      case 'history':
        renderHistoryView(mainViewEl);
        break;
      case 'accounts':
        renderAccountsView(mainViewEl);
        break;
      case 'settings':
        renderSettingsView(mainViewEl);
        break;
      default:
        renderChatView(mainViewEl);
    }

    // 3. Task Panel (only on chats page or when plan exists)
    if (state.activePage === 'chats' || state.tasksOpen) {
      taskPanelEl.style.display = 'block';
      renderTaskPanel(taskPanelEl);
    } else {
      taskPanelEl.style.display = 'none';
    }

    // 4. Animation Lab Modal
    renderAnimationLab(modalEl);
  }

  // Subscribe to reactive state updates
  store.subscribe(() => {
    render();
  });

  // Initial render
  render();

  // Keyboard Shortcuts: Esc to stop execution
  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      simulation.stopRun();
    }
  });

  console.log("👻 SetupPal Zero-Trust GUI initialized. Ready in Cozy Kawaii Chill Mode.");
}

// Start app on DOM load
window.addEventListener('DOMContentLoaded', initApp);
