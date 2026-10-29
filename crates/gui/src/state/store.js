/**
 * SetupPal: Central Reactive State Store
 */

import { sounds } from '../audio/sound-engine.js';
import { petLine } from '../mascot/pet-lines.js';

const STORAGE_KEY = 'setuppal_local_conversations_v2';
const ACTIVE_CONV_KEY = 'setuppal_active_conv_id_v2';

function generateConvId() {
  return 'conv_' + Date.now() + '_' + Math.random().toString(36).substring(2, 6);
}

function createNewConversation(title = 'New setup', icon = '✨') {
  return {
    id: generateConvId(),
    title,
    icon,
    createdAt: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
    chatMessages: [],
    activePlan: null,
    terminalLogs: [
      { id: 1, text: '> Zero-Trust Orchestrator ready. Standing by for user request...', type: 'info' }
    ]
  };
}

function loadStoredConversations() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed) && parsed.length > 0) {
        return parsed;
      }
    }
  } catch (e) {
    console.warn("Could not parse local conversations:", e);
  }
  const defaultConv = createNewConversation('New setup', '✨');
  saveStoredConversations([defaultConv], defaultConv.id);
  return [defaultConv];
}

function saveStoredConversations(conversations, activeId) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(conversations));
    if (activeId) {
      localStorage.setItem(ACTIVE_CONV_KEY, activeId);
    }
  } catch (e) {
    console.warn("Could not save local conversations:", e);
  }
}

class Store {
  constructor() {
    this.subscribers = new Set();
    
    const conversations = loadStoredConversations();
    let activeId = localStorage.getItem(ACTIVE_CONV_KEY);
    let activeConv = conversations.find(c => c.id === activeId);
    if (!activeConv) {
      activeConv = conversations[0];
      activeId = activeConv.id;
    }

    this.state = {
      mode: 'chill', // 'chill' | 'hacker'
      modePreference: 'auto', // 'auto' | 'always-chill' | 'always-hacker'
      activePage: 'chats', // 'chats' | 'history' | 'downloads' | 'accounts' | 'settings'
      
      // Conversations & Tabs (Persisted locally)
      conversations: conversations,
      activeConversationId: activeId,

      // Mascot & Pets
      activePet: 'ghost', // 'ghost' | 'cat' | 'bunny'
      mascotState: 'idle',
      mascotSpeech: petLine('ghost', 'welcome'),
      mascotSpeechSpeed: 25, // ms per char
      isSpeaking: false,
      
      // Right Task Panel
      tasksOpen: true,
      
      // Active Plan & Tasks (restored from active conversation)
      activePlan: activeConv.activePlan || null,

      // Terminal Logs (restored from active conversation)
      terminalTab: 'terminal', // 'terminal' | 'chat'
      terminalLogs: activeConv.terminalLogs || [
        { id: 1, text: '> Zero-Trust Orchestrator ready. Standing by for user request...', type: 'info' }
      ],

      // Chat Messages (restored from active conversation)
      chatMessages: activeConv.chatMessages || [],

      // RECENT list in sidebar (synced with conversations)
      recentChats: conversations.map(c => ({
        id: c.id,
        title: c.title,
        icon: c.icon,
        active: c.id === activeId
      })),

      // Downloads Data
      // Real downloads, derived from the backend transaction log
      downloads: [],

      // History Data (filled from the backend transaction log)
      history: {
        transactions: [],
        loaded: false,
        error: null
      },

      // Settings
      settings: {
        safetyLevel: 'strict', // 'strict' | 'balanced' | 'relaxed'
        textScale: 'm', // 's' | 'm' | 'l'
        reduceMotion: false,
        mascotAnimations: true,
        speechBubbles: true,
        soundEnabled: true,
        masterVolume: 0.35,
        modelPlanner: 'NVIDIA Nemotron-3 70B (Planning & Safety)',
        modelVerification: 'Local Deterministic Safety Engine'
      },

      // Accounts (Local credentials & state)
      accounts: {
        displayName: localStorage.getItem('setuppal_display_name') || 'Local User',
        avatarColor: 'lavender',
        githubConnected: !!localStorage.getItem('setuppal_github_user'),
        githubUsername: localStorage.getItem('setuppal_github_user') || '',
        nebiusConnected: false,
        tavilyConnected: !!localStorage.getItem('setuppal_tavily_key'),
        stats: {
          messages: 0,
          toolsExecuted: 0,
          hashesChecked: 0,
          quarantined: 0
        }
      },

      // Mascot Lab Modal
      animationLabOpen: false
    };
  }

  getState() {
    return this.state;
  }

  subscribe(callback) {
    this.subscribers.add(callback);
    return () => this.subscribers.delete(callback);
  }

  notify() {
    for (const callback of this.subscribers) {
      callback(this.state);
    }
  }

  setState(updater) {
    if (typeof updater === 'function') {
      this.state = updater(this.state);
    } else {
      this.state = { ...this.state, ...updater };
    }

    // Memory and storage guard: cap terminal logs to sliding window of 100 items
    if (this.state.terminalLogs && this.state.terminalLogs.length > 100) {
      this.state.terminalLogs = this.state.terminalLogs.slice(-100);
    }

    // Auto-persist active conversation if messages, plan, or logs changed
    if (this.state.activeConversationId && Array.isArray(this.state.conversations)) {
      let changed = false;
      const convs = this.state.conversations.map(c => {
        if (c.id === this.state.activeConversationId) {
          if (c.chatMessages !== this.state.chatMessages || c.activePlan !== this.state.activePlan || c.terminalLogs !== this.state.terminalLogs) {
            changed = true;
            return {
              ...c,
              chatMessages: this.state.chatMessages,
              activePlan: this.state.activePlan,
              terminalLogs: (this.state.terminalLogs || []).slice(-100),
              updatedAt: Date.now()
            };
          }
        }
        return c;
      });
      if (changed) {
        this.state.conversations = convs;
        saveStoredConversations(convs, this.state.activeConversationId);
      }
    }

    this.notify();
  }

  createConversation() {
    // 1. Save currently active conversation first
    const s = this.state;
    const updatedConvs = (s.conversations || []).map(c => {
      if (c.id === s.activeConversationId) {
        return {
          ...c,
          chatMessages: s.chatMessages,
          activePlan: s.activePlan,
          terminalLogs: s.terminalLogs,
          updatedAt: Date.now()
        };
      }
      return c;
    });

    // 2. Create fresh new conversation
    const newConv = createNewConversation('New setup', '✨');
    const allConvs = [newConv, ...updatedConvs];

    saveStoredConversations(allConvs, newConv.id);

    this.setState({
      conversations: allConvs,
      activeConversationId: newConv.id,
      chatMessages: [],
      activePlan: null,
      terminalLogs: newConv.terminalLogs,
      activePage: 'chats',
      recentChats: allConvs.map(c => ({
        id: c.id,
        title: c.title,
        icon: c.icon,
        active: c.id === newConv.id
      }))
    });

    sounds.play('ui.click');
    this.setMascotState('idle', petLine(this.state.activePet || 'ghost', 'new_chat'));
  }

  switchConversation(id) {
    if (this.state.activeConversationId === id) return;

    // 1. Save currently active conversation
    const s = this.state;
    const updatedConvs = (s.conversations || []).map(c => {
      if (c.id === s.activeConversationId) {
        return {
          ...c,
          chatMessages: s.chatMessages,
          activePlan: s.activePlan,
          terminalLogs: s.terminalLogs,
          updatedAt: Date.now()
        };
      }
      return c;
    });

    const targetConv = updatedConvs.find(c => c.id === id);
    if (!targetConv) return;

    saveStoredConversations(updatedConvs, targetConv.id);

    this.setState({
      conversations: updatedConvs,
      activeConversationId: targetConv.id,
      chatMessages: targetConv.chatMessages || [],
      activePlan: targetConv.activePlan || null,
      terminalLogs: targetConv.terminalLogs || [
        { id: 1, text: '> Zero-Trust Orchestrator ready. Standing by for user request...', type: 'info' }
      ],
      activePage: 'chats',
      recentChats: updatedConvs.map(c => ({
        id: c.id,
        title: c.title,
        icon: c.icon,
        active: c.id === targetConv.id
      }))
    });

    sounds.play('ui.click');
    this.setMascotState('idle', petLine(this.state.activePet || 'ghost', 'switch_chat'));
  }

  closeConversation(id) {
    const s = this.state;
    const remaining = (s.conversations || []).filter(c => c.id !== id);

    if (remaining.length === 0) {
      this.createConversation();
      return;
    }

    let nextActiveId = s.activeConversationId;
    let nextConv = remaining.find(c => c.id === nextActiveId);

    if (s.activeConversationId === id || !nextConv) {
      nextConv = remaining[0];
      nextActiveId = nextConv.id;
    }

    saveStoredConversations(remaining, nextActiveId);

    this.setState({
      conversations: remaining,
      activeConversationId: nextActiveId,
      chatMessages: nextConv.chatMessages || [],
      activePlan: nextConv.activePlan || null,
      terminalLogs: nextConv.terminalLogs || [],
      recentChats: remaining.map(c => ({
        id: c.id,
        title: c.title,
        icon: c.icon,
        active: c.id === nextActiveId
      }))
    });

    sounds.play('ui.click');
  }

  updateConversationTitleFromMessage(userText) {
    const s = this.state;
    const active = (s.conversations || []).find(c => c.id === s.activeConversationId);
    if (!active) return;

    // Only update if current title is default
    if (active.title === 'New setup' || active.title.startsWith('New setup')) {
      const lower = userText.toLowerCase();
      let newTitle = userText.slice(0, 24);
      let newIcon = '💬';

      if (lower.includes('web') || lower.includes('node') || lower.includes('typescript') || lower.includes('javascript')) {
        newTitle = 'Web & TypeScript';
        newIcon = '🌐';
      } else if (lower.includes('rust') || lower.includes('cargo')) {
        newTitle = 'Rust toolchain';
        newIcon = '🦀';
      } else if (lower.includes('python') || lower.includes('venv') || lower.includes('data science')) {
        newTitle = 'Python AI dev';
        newIcon = '🐍';
      } else if (lower.includes('c++') || lower.includes('cpp') || lower.includes('compiler')) {
        newTitle = 'C++ toolchain';
        newIcon = '⚡';
      } else if (lower.includes('attack') || lower.includes('radar')) {
        newTitle = 'Security Radar';
        newIcon = '🛡️';
      }

      const updatedConvs = s.conversations.map(c => {
        if (c.id === s.activeConversationId) {
          return { ...c, title: newTitle, icon: newIcon };
        }
        return c;
      });

      saveStoredConversations(updatedConvs, s.activeConversationId);

      this.setState({
        conversations: updatedConvs,
        recentChats: updatedConvs.map(c => ({
          id: c.id,
          title: c.title,
          icon: c.icon,
          active: c.id === s.activeConversationId
        }))
      });
    }
  }

  setMode(mode) {
    if (this.state.mode === mode) return;
    const prev = this.state.mode;
    this.setState({ mode });
    document.documentElement.dataset.mode = mode;
    
    // Play transition sound
    if (mode === 'hacker') {
      sounds.play('mode.hacker');
    } else {
      sounds.play('mode.chill');
    }
  }

  setMascotState(state, speech = null) {
    this.state.mascotState = state;
    if (speech !== null) {
      this.state.mascotSpeech = speech;
    }
    
    // Update DOM speech bubble and mascot element directly without destroying user input / DOM
    const speechEl = document.getElementById('speech-bubble-text');
    if (speechEl && speech !== null) {
      speechEl.textContent = speech;
    }
    const mascotSvg = document.getElementById('hero-ghost-root') || document.getElementById('ghost-root');
    if (mascotSvg) {
      if (mascotSvg.dataset.state === state) {
        mascotSvg.dataset.state = '';
        void mascotSvg.offsetWidth;
      }
      mascotSvg.dataset.state = state;
    }
  }

  setActivePet(pet) {
    pet = 'ghost';
    this.setState({ activePet: pet });
    sounds.play('ui.click');
    
    // Greet with pet's personality
    this.setMascotState('wave-hello', petLine(pet, 'welcome'));
  }

  setActivePage(page) {
    this.setState({ activePage: page });
    sounds.play('ui.click');
  }

  toggleAnimationLab(open) {
    this.setState({ animationLabOpen: open !== undefined ? open : !this.state.animationLabOpen });
    sounds.play('ui.click');
  }
}

export const store = new Store();
