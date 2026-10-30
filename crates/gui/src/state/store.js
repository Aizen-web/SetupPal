/**
 * SetupPal: Central Reactive State Store
 */

import { sounds } from '../audio/sound-engine.js';
import { normalizeMascotState } from '../mascot/animations.js';
import { pixelLine } from '../mascot/pixel-lines.js';
import { getUiSettings, setUiSettings, TEXT_SIZES } from './prefs.js';

const STORAGE_KEY = 'setuppal_local_conversations_v2';
const ACTIVE_CONV_KEY = 'setuppal_active_conv_id_v2';
const LEGACY_PET_KEY = 'setuppal_active_pet';
const DEFAULT_TITLE = 'New setup';

/** Minimum time in a mode before auto mode switches back (docs/UI.md 2.3). */
export const MODE_MIN_DWELL_MS = 2000;

function generateConvId() {
  return 'conv_' + Date.now() + '_' + Math.random().toString(36).substring(2, 6);
}

function createNewConversation(title = DEFAULT_TITLE) {
  return {
    id: generateConvId(),
    title,
    createdAt: Date.now(),
    updatedAt: Date.now(),
    chatMessages: [],
    activePlan: null,
    manualGuide: null,
    packageConfirm: null,
    terminalLogs: [],
  };
}

function loadStoredConversations() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed) && parsed.length > 0) {
        return parsed.map(conversation => ({
          ...conversation,
          chatMessages: (conversation.chatMessages || []).map(message =>
            message.isStreaming
              ? { ...message, isStreaming: false, stopped: true }
              : message
          ),
        }));
      }
    }
  } catch (e) {
    console.warn("Could not parse local conversations:", e);
  }
  const defaultConv = createNewConversation();
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

function toRecent(conversations, activeId) {
  return conversations.map(c => ({ id: c.id, title: c.title, active: c.id === activeId }));
}

/** Mirrors settings that CSS reads onto <html>. */
function applyDocumentSettings(settings) {
  const root = document.documentElement;
  root.dataset.bgIntensity = settings.backgroundIntensity;
  root.dataset.textSize = settings.textScale;
  root.dataset.reduceMotion = String(settings.reduceMotion);
  root.style.setProperty('--text-zoom', String((TEXT_SIZES[settings.textScale] || TEXT_SIZES.m).zoom));
}

class Store {
  constructor() {
    this.subscribers = new Set();
    localStorage.removeItem(LEGACY_PET_KEY);

    const conversations = loadStoredConversations();
    let activeId = localStorage.getItem(ACTIVE_CONV_KEY);
    let activeConv = conversations.find(c => c.id === activeId);
    if (!activeConv) {
      activeConv = conversations[0];
      activeId = activeConv.id;
    }

    const settings = getUiSettings();
    const initialMode = settings.modePreference === 'always-hacker' ? 'hacker' : 'chill';
    document.documentElement.dataset.mode = initialMode;
    applyDocumentSettings(settings);
    sounds.setVolume(settings.masterVolume);
    sounds.setMuted(!settings.soundEnabled);

    // Mode controller (not rendered state)
    this.modeEnteredAt = Date.now();
    this.modeOverride = null; // 'chill' | 'hacker' while a manual /hack or /chill is in force
    this.runActive = false;
    this.modeTimer = null;

    this.state = {
      mode: initialMode, // 'chill' | 'hacker'
      modePreference: settings.modePreference, // 'auto' | 'always-chill' | 'always-hacker'
      activePage: 'chats', // 'chats' | 'history' | 'downloads' | 'accounts' | 'settings'

      conversations,
      activeConversationId: activeId,

      mascotState: 'idle',
      mascotSpeech: pixelLine('welcome'),

      // Right task panel: user can collapse it while a plan exists
      tasksOpen: true,

      // Active plan and terminal log (restored from the active conversation)
      activePlan: activeConv.activePlan || null,
      manualGuide: activeConv.manualGuide || null,
      packageConfirm: activeConv.packageConfirm || null,
      // { conversationId, label } while a plan is being generated; never persisted
      planPending: null,
      // When true, chat-view scrolls the plan card into view without jumping to the top (R23).
      scrollToPlan: false,
      terminalTab: 'terminal', // 'terminal' | 'chat' (Hacker mode tab strip)
      terminalLogs: activeConv.terminalLogs || [],

      chatMessages: activeConv.chatMessages || [],
      isChatGenerating: (activeConv.chatMessages || []).some(message => message.isStreaming),

      recentChats: toRecent(conversations, activeId),

      // Derived from the backend transaction log
      downloads: [],
      history: {
        transactions: [],
        loaded: false,
        error: null
      },

      settings,

      // Accounts (owned by the Accounts view)
      accounts: {
        displayName: localStorage.getItem('setuppal_display_name') || 'Local User',
        avatarColor: 'lavender',
        // Key status comes from the backend (OS keychain); nothing key-like is kept in localStorage.
        githubConnected: false,
        nebiusConnected: false,
        tavilyConnected: false,
        stats: {
          messages: 0,
          toolsExecuted: 0,
          hashesChecked: 0,
          quarantined: 0
        }
      },

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

  updateConversationMessage(conversationId, messageId, patch) {
    const updateMessages = (messages = []) => messages.map(message =>
      message.id === messageId ? { ...message, ...patch } : message
    );
    this.state.conversations = this.state.conversations.map(conversation =>
      conversation.id === conversationId
        ? { ...conversation, chatMessages: updateMessages(conversation.chatMessages) }
        : conversation
    );
    if (this.state.activeConversationId === conversationId) {
      this.state.chatMessages = updateMessages(this.state.chatMessages);
    }
  }

  getConversationMessages(conversationId) {
    return this.state.conversations.find(conversation => conversation.id === conversationId)?.chatMessages || [];
  }

  appendConversationMessage(conversationId, message) {
    this.state.conversations = this.state.conversations.map(conversation =>
      conversation.id === conversationId
        ? { ...conversation, chatMessages: [...(conversation.chatMessages || []), message] }
        : conversation
    );
    if (this.state.activeConversationId === conversationId) {
      this.setState(s => ({
        ...s,
        chatMessages: [...s.chatMessages, message],
      }));
    } else {
      this.persistConversations();
    }
  }

  persistConversations() {
    saveStoredConversations(this.state.conversations, this.state.activeConversationId);
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
          if (c.chatMessages !== this.state.chatMessages || c.activePlan !== this.state.activePlan || c.manualGuide !== this.state.manualGuide || c.packageConfirm !== this.state.packageConfirm || c.terminalLogs !== this.state.terminalLogs) {
            changed = true;
            return {
              ...c,
              chatMessages: this.state.chatMessages,
              activePlan: this.state.activePlan,
              manualGuide: this.state.manualGuide,
              packageConfirm: this.state.packageConfirm,
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

  /** Copies the live chat state back into its conversation record. */
  snapshotActiveConversation() {
    const s = this.state;
    return (s.conversations || []).map(c => (
      c.id === s.activeConversationId
        ? { ...c, chatMessages: s.chatMessages, activePlan: s.activePlan, manualGuide: s.manualGuide, packageConfirm: s.packageConfirm, terminalLogs: s.terminalLogs, updatedAt: Date.now() }
        : c
    ));
  }

  createConversation() {
    const newConv = createNewConversation();
    const allConvs = [newConv, ...this.snapshotActiveConversation()];
    saveStoredConversations(allConvs, newConv.id);

    this.setState({
      conversations: allConvs,
      activeConversationId: newConv.id,
      chatMessages: [],
      isChatGenerating: false,
      activePlan: null,
      manualGuide: null,
      packageConfirm: null,
      terminalLogs: newConv.terminalLogs,
      activePage: 'chats',
      recentChats: toRecent(allConvs, newConv.id),
    });

    sounds.play('ui.click');
    this.setMascotState('wave-hello', pixelLine('new_chat'));
  }

  switchConversation(id) {
    if (this.state.activeConversationId === id) {
      if (this.state.activePage !== 'chats') this.setActivePage('chats');
      return;
    }

    const updatedConvs = this.snapshotActiveConversation();
    const targetConv = updatedConvs.find(c => c.id === id);
    if (!targetConv) return;

    saveStoredConversations(updatedConvs, targetConv.id);

    this.setState({
      conversations: updatedConvs,
      activeConversationId: targetConv.id,
      chatMessages: targetConv.chatMessages || [],
      isChatGenerating: (targetConv.chatMessages || []).some(message => message.isStreaming),
      activePlan: targetConv.activePlan || null,
      manualGuide: targetConv.manualGuide || null,
      packageConfirm: targetConv.packageConfirm || null,
      terminalLogs: targetConv.terminalLogs || [],
      activePage: 'chats',
      recentChats: toRecent(updatedConvs, targetConv.id),
    });

    sounds.play('ui.click');
    this.setMascotState('idle', pixelLine('switch_chat'));
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
      isChatGenerating: (nextConv.chatMessages || []).some(message => message.isStreaming),
      activePlan: nextConv.activePlan || null,
      manualGuide: nextConv.manualGuide || null,
      packageConfirm: nextConv.packageConfirm || null,
      terminalLogs: nextConv.terminalLogs || [],
      recentChats: toRecent(remaining, nextActiveId),
    });

    sounds.play('ui.click');
  }

  updateConversationTitleFromMessage(userText) {
    const s = this.state;
    const active = (s.conversations || []).find(c => c.id === s.activeConversationId);
    if (!active || !active.title.startsWith(DEFAULT_TITLE)) return;

    const lower = userText.toLowerCase();
    let newTitle = userText.replace(/\s+/g, ' ').trim().slice(0, 32);
    if (/\b(web|node|typescript|javascript)\b/.test(lower)) newTitle = 'Web & TypeScript';
    else if (/\b(rust|cargo)\b/.test(lower)) newTitle = 'Rust toolchain';
    else if (/\b(python|venv)\b|data science/.test(lower)) newTitle = 'Python setup';
    else if (/c\+\+|\bcpp\b|compiler/.test(lower)) newTitle = 'C++ toolchain';
    if (!newTitle) return;

    const updatedConvs = s.conversations.map(c => (c.id === s.activeConversationId ? { ...c, title: newTitle } : c));
    saveStoredConversations(updatedConvs, s.activeConversationId);
    this.setState({
      conversations: updatedConvs,
      recentChats: toRecent(updatedConvs, s.activeConversationId),
    });
  }

  /* ---------------------------------------------------------------- */
  /* Modes                                                             */
  /* ---------------------------------------------------------------- */

  /** Low-level switch. Prefer beginRun/endRun/setModeManual so auto mode stays consistent. */
  setMode(mode) {
    if (this.state.mode === mode) return;
    this.modeEnteredAt = Date.now();
    document.documentElement.dataset.mode = mode;
    this.setState({ mode });
    sounds.play(mode === 'hacker' ? 'mode.hacker' : 'mode.chill');
  }

  clearModeTimer() {
    if (this.modeTimer) clearTimeout(this.modeTimer);
    this.modeTimer = null;
  }

  /** /hack, /chill, and the header toggle: forced until the next run ends. */
  setModeManual(mode) {
    this.clearModeTimer();
    this.modeOverride = mode;
    this.setMode(mode);
  }

  /** A plan run started (first task running). Auto mode goes to Hacker and shows the terminal. */
  beginRun() {
    this.runActive = true;
    this.clearModeTimer();
    if (this.state.modePreference !== 'auto' || this.modeOverride) return;
    if (this.state.terminalTab !== 'terminal') this.setState({ terminalTab: 'terminal' });
    this.setMode('hacker');
  }

  /** A plan run finished or stopped. Auto mode returns to Chill after the minimum dwell. */
  endRun() {
    this.runActive = false;
    this.modeOverride = null;
    this.clearModeTimer();
    if (this.state.modePreference !== 'auto' || this.state.mode === 'chill') return;
    const wait = Math.max(0, MODE_MIN_DWELL_MS - (Date.now() - this.modeEnteredAt));
    this.modeTimer = setTimeout(() => {
      this.modeTimer = null;
      if (!this.runActive && !this.modeOverride && this.state.modePreference === 'auto') this.setMode('chill');
    }, wait);
  }

  setModePreference(pref) {
    this.updateSettings({ modePreference: pref });
    this.modeOverride = null;
    this.clearModeTimer();
    if (pref === 'always-chill') this.setMode('chill');
    else if (pref === 'always-hacker') this.setMode('hacker');
    else this.setMode(this.runActive ? 'hacker' : 'chill');
  }

  setTerminalTab(tab) {
    if (tab !== 'terminal' && tab !== 'chat') return;
    this.setState({ terminalTab: tab });
    sounds.play('ui.click');
  }

  /* ---------------------------------------------------------------- */
  /* Settings                                                          */
  /* ---------------------------------------------------------------- */

  updateSettings(patch) {
    const settings = setUiSettings(patch);
    applyDocumentSettings(settings);
    sounds.setVolume(settings.masterVolume);
    sounds.setMuted(!settings.soundEnabled);
    this.setState({ settings, modePreference: settings.modePreference });
  }

  /* ---------------------------------------------------------------- */
  /* Mascot                                                            */
  /* ---------------------------------------------------------------- */

  setMascotState(state, speech = null) {
    const nextState = normalizeMascotState(state);
    this.state.mascotState = nextState;
    if (speech !== null) {
      this.state.mascotSpeech = speech;
    }

    // Update the live mascot and bubble in place so typing and scrolling are not disturbed.
    const speechEl = document.getElementById('speech-bubble-text');
    if (speechEl && speech !== null) {
      speechEl.textContent = speech;
    }
    const mascotSvg = document.querySelector('[data-mascot-live="true"]');
    if (mascotSvg) {
      if (mascotSvg.dataset.state === nextState) {
        mascotSvg.dataset.state = '';
        void mascotSvg.getBoundingClientRect();
      }
      mascotSvg.dataset.state = nextState;
    }
  }

  setActivePage(page) {
    this.setState({ activePage: page });
    sounds.play('ui.click');
  }

  setTasksOpen(open) {
    this.setState({ tasksOpen: Boolean(open) });
    sounds.play('ui.click');
  }

  toggleAnimationLab(open) {
    this.setState({ animationLabOpen: open !== undefined ? open : !this.state.animationLabOpen });
    sounds.play('ui.click');
  }
}

export const store = new Store();
