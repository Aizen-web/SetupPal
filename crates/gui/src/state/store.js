/**
 * SetupPal: Central Reactive State Store
 */

import { sounds } from '../audio/sound-engine.js';

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
      mascotSpeech: "Hey there! I'm Setup Pal.\nI'll help you set up your development\ntools and environments safely!",
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
      downloads: [
        {
          id: 'd1',
          filename: 'OpenJDK21U-jdk_x64_windows_hotspot_21.0.3_9.msi',
          source: 'adoptium.net',
          size: '189.4 MB',
          sha256: 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
          hashStatus: 'matches publisher',
          verdict: 'GREEN',
          date: '10 mins ago',
          status: 'verified'
        },
        {
          id: 'd2',
          filename: 'fabric-installer-1.0.1.exe',
          source: 'fabricmc.net',
          size: '18.0 MB',
          sha256: '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
          hashStatus: 'downloading (78%)',
          verdict: 'GREEN',
          date: 'Just now',
          status: 'queue',
          progress: 78,
          speed: '4.8 MB/s',
          eta: '2s'
        },
        {
          id: 'd3',
          filename: 'sodium-fabric-mc1.21-0.5.11.jar',
          source: 'modrinth.com',
          size: '1.2 MB',
          sha256: '5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8',
          hashStatus: 'matches publisher',
          verdict: 'GREEN',
          date: 'Queued',
          status: 'queue',
          progress: 0
        },
        {
          id: 'd4',
          filename: 'fastlight-pro-v0.1.jar',
          source: 'modrinth.com',
          size: '640 KB',
          sha256: '4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a',
          hashStatus: 'held for review',
          verdict: 'YELLOW',
          date: 'Awaiting call',
          status: 'quarantine'
        }
      ],

      // History Data
      history: {
        conversations: [
          { id: 'h1', title: 'Minecraft + mods', date: 'Today, 14:20', type: 'Setup', tasksDone: '2 of 4', verdict: 'YELLOW' },
          { id: 'h2', title: 'C++ toolchain', date: 'Yesterday, 19:15', type: 'Dev Environment', tasksDone: '4 of 4', verdict: 'GREEN' },
          { id: 'h3', title: 'Python data science', date: '2 days ago', type: 'Venv Setup', tasksDone: '3 of 3', verdict: 'GREEN' },
          { id: 'h4', title: 'Typosquat Security Demo', date: '3 days ago', type: 'Audit', tasksDone: '1 blocked', verdict: 'RED' }
        ],
        changes: [
          { id: 'tx-103', time: '14:20:48', desc: 'Installed Adoptium Temurin 21 JDK (winget)', reversible: 'Full', task: 'Install Java 21' },
          { id: 'tx-102', time: '14:20:12', desc: 'Added JAVA_HOME to HKCU\\Environment', reversible: 'Full', task: 'Install Java 21' },
          { id: 'tx-101', time: '14:20:05', desc: 'Created Minecraft data folder (%APPDATA%\\.minecraft)', reversible: 'Full', task: 'Install Launcher' }
        ]
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
    this.setMascotState('idle', "Started a fresh new setup! Ask me anything in the chat window, and I'll help you set it up safely!");
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
    this.setMascotState('idle', `Switched back to "${targetConv.title}"! Standing by with zero-trust protection.`);
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

      if (lower.includes('minecraft') || lower.includes('optifine')) {
        newTitle = 'Minecraft mods';
        newIcon = '🎮';
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
    if (!['ghost', 'cat', 'bunny'].includes(pet)) return;
    this.setState({ activePet: pet });
    sounds.play('ui.click');
    
    // Greet with pet's personality
    if (pet === 'cat') {
      this.setMascotState('wave-hello', "Purrr! I'm Byte the Cyber Cat. Ask me anything in the chat window, and I'll keep your machine safe!");
    } else if (pet === 'bunny') {
      this.setMascotState('wave-hello', "Binky! I'm Mochi the Bunny. Ask me anything in the chat window and we'll set it up safely!");
    } else {
      this.setMascotState('wave-hello', "Boo! I'm Pixel, your zero-trust guardian ghost. Ask me anything in the chat window, and I'll help you set it up safely!");
    }
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
