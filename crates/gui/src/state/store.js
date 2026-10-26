/**
 * SetupPal: Central Reactive State Store
 */

import { sounds } from '../audio/sound-engine.js';

class Store {
  constructor() {
    this.subscribers = new Set();
    
    this.state = {
      mode: 'chill', // 'chill' | 'hacker'
      modePreference: 'auto', // 'auto' | 'always-chill' | 'always-hacker'
      activePage: 'chats', // 'chats' | 'history' | 'downloads' | 'accounts' | 'settings'
      
      // Mascot & Pets
      activePet: 'ghost', // 'ghost' | 'cat' | 'bunny'
      mascotState: 'idle',
      mascotSpeech: "Here's my plan. Take a look before I touch anything.",
      mascotSpeechSpeed: 25, // ms per char
      isSpeaking: false,
      
      // Right Task Panel
      tasksOpen: true,
      
      // Active Plan & Tasks
      activePlan: {
        id: 'minecraft_01',
        title: 'Minecraft with performance mods',
        subtitle: '5 steps · 4 look safe · 1 needs your call',
        safety: 'strict',
        status: 'awaiting_approval', // 'planning' | 'awaiting_approval' | 'running' | 'completed' | 'paused'
        tasks: [
          {
            id: 1,
            num: 1,
            title: 'Install Java 21',
            desc: 'adoptium.net · hash matches publisher',
            source: 'adoptium.net',
            verdict: 'GREEN',
            status: 'done',
            reversible: true
          },
          {
            id: 2,
            num: 2,
            title: 'Install the Minecraft launcher',
            desc: 'minecraft.net · signature valid',
            source: 'minecraft.net',
            verdict: 'GREEN',
            status: 'done',
            reversible: true
          },
          {
            id: 3,
            num: 3,
            title: 'Install Fabric loader',
            desc: 'fabricmc.net · official installer',
            source: 'fabricmc.net',
            verdict: 'GREEN',
            status: 'running',
            reversible: true
          },
          {
            id: 4,
            num: 4,
            title: 'Add Sodium',
            desc: 'modrinth.com · established, hash matches',
            source: 'modrinth.com',
            verdict: 'GREEN',
            status: 'waiting',
            reversible: true
          },
          {
            id: 5,
            num: 5,
            title: 'Add FastLight Pro',
            desc: 'modrinth.com · brand new, few downloads',
            source: 'modrinth.com',
            verdict: 'YELLOW',
            status: 'held',
            reversible: false
          }
        ]
      },

      // Terminal Logs
      terminalTab: 'terminal', // 'terminal' | 'chat'
      terminalLogs: [
        { id: 1, text: '> plan approved: 4 steps, 1 held for you', type: 'info' },
        { id: 2, text: '✓ java 21 · adoptium.net · sha256 matches publisher', type: 'green' },
        { id: 3, text: '✓ launcher · minecraft.net · signature valid', type: 'green' },
        { id: 4, text: '~ fabric loader · downloading (14.2 MB / 18.0 MB)', type: 'running' },
        { id: 5, text: '> next: sodium · modrinth · hash check', type: 'info' },
        { id: 6, text: '? FastLight Pro is held (yellow). Ask me later.', type: 'yellow' }
      ],

      // Chat Messages
      chatMessages: [
        {
          id: 'm1',
          sender: 'user',
          text: 'Set up Minecraft with performance mods for 1.21 please',
          time: '14:20'
        },
        {
          id: 'm2',
          sender: 'assistant',
          text: "I analyzed the dependencies and security risks for **Minecraft 1.21 + Fabric**. Adoptium Temurin 21 and the official launcher have verified cryptographic signatures.\n\nSodium has over 30M verified downloads. Note that I flagged **FastLight Pro** as yellow because it was uploaded recently and lacks historical trust metrics.",
          time: '14:21',
          planAttached: true
        }
      ],

      // RECENT list in sidebar
      recentChats: [
        { id: 'c1', title: 'Minecraft + mods', active: true, status: 'running', icon: '~' },
        { id: 'c2', title: 'C++ toolchain', active: false, status: 'done', icon: '✓' },
        { id: 'c3', title: 'Python data science', active: false, status: 'done', icon: '✓' },
        { id: 'c4', title: 'Node project', active: false, status: 'done', icon: '✓' }
      ],

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

      // Accounts
      accounts: {
        displayName: 'Ghost Pilot',
        avatarColor: 'lavender',
        githubConnected: true,
        githubUsername: 'Aizen-web',
        nebiusConnected: true,
        tavilyConnected: true,
        stats: {
          messages: 48,
          toolsExecuted: 31,
          hashesChecked: 19,
          quarantined: 2
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
    this.notify();
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
    const updates = { mascotState: state };
    if (speech !== null) {
      updates.mascotSpeech = speech;
    }
    this.setState(updates);
    
    // Update DOM data-state attribute
    const mascotRoot = document.getElementById('ghost-root');
    if (mascotRoot) {
      mascotRoot.dataset.state = state;
    }
  }

  setActivePet(pet) {
    if (!['ghost', 'cat', 'bunny'].includes(pet)) return;
    this.setState({ activePet: pet });
    sounds.play('ui.click');
    
    // Greet with pet's personality
    if (pet === 'cat') {
      this.setMascotState('wave-hello', "Meow! Byte the cyber-cat is online and monitoring processes! :3");
    } else if (pet === 'bunny') {
      this.setMascotState('wave-hello', "Hop-hop! Mochi the bunny is here to keep your setup safe and cozy! 🥕");
    } else {
      this.setMascotState('wave-hello', "Boo... I mean, hi! Pixel the ghost is ready to protect your machine!");
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
