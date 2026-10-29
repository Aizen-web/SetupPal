/**
 * SetupPal: Backend-to-Frontend State Bridge
 * Coordinates between Rust installer_core API, EventBus SSE stream, and the reactive UI Store.
 */

import { apiClient } from './client.js';
import { store } from '../state/store.js';
import { setLastContext } from '../state/prefs.js';
import { sounds } from '../audio/sound-engine.js';
import { petVoiceEngine } from '../mascot/voicelines.js';
import confetti from 'canvas-confetti';

function formatStepDetails(step) {
  if (!step) return { desc: 'Zero-trust verified step', source: 'local' };
  
  if (step.action === 'install_package') {
    const pkg = step.params.package_id;
    const eco = step.params.ecosystem || 'winget';
    return {
      desc: `${pkg} via ${eco} · deterministic package install`,
      source: `${eco}`
    };
  }
  if (step.action === 'download_artifact') {
    const dest = step.params.destination || step.params.id;
    const sha = step.params.expected_sha256 ? `${step.params.expected_sha256.slice(0, 8)}...` : 'hash checked';
    return {
      desc: `${dest} · SHA-256: ${sha}`,
      source: step.params.adapter || 'official_vendor'
    };
  }
  if (step.action === 'verify_hash') {
    return {
      desc: `Verify cryptographic SHA-256 digest on ${step.params.file_path}`,
      source: 'sha256_verifier'
    };
  }
  if (step.action === 'add_to_user_path') {
    return {
      desc: `Add ${step.params.directory} to Windows User Environment PATH (HKCU)`,
      source: 'windows_registry'
    };
  }
  if (step.action === 'create_isolated_env') {
    return {
      desc: `Create sandbox isolated environment at ${step.params.path}`,
      source: 'sandbox'
    };
  }
  if (step.action === 'run_verify_command') {
    const bin = step.params.binary || 'verify';
    const args = (step.params.args || []).join(' ');
    return {
      desc: `Run verified test: ${bin} ${args}`,
      source: `${bin}`
    };
  }
  return { desc: 'Standard verified step', source: 'orchestrator' };
}

export class BackendBridge {
  constructor() {
    this.isBackendOnline = false;
    this.activeExecutionPlan = null;
    this.chatControllers = new Map();
    this.chatQueues = new Map();
    this._connectionPromise = new Promise(resolve => {
      this._resolveConnection = resolve;
    });
  }

  async waitForConnection(timeoutMs = 4000) {
    if (this.isBackendOnline) return true;
    let timer;
    const timeout = new Promise(resolve => {
      timer = setTimeout(() => resolve(false), timeoutMs);
    });
    const ok = await Promise.race([this._connectionPromise, timeout]);
    clearTimeout(timer);
    return !!ok;
  }

  async init(attempt = 0) {
    const health = await apiClient.checkHealth(1200);
    if (health) {
      this.isBackendOnline = true;
      if (this._resolveConnection) {
        this._resolveConnection(true);
      }
      console.log("Zero-Trust Backend Connected:", health);
      this.setupEventStream();
      this.refreshHistory();
    } else {
      // Rapid retry with progressive backoff: 80ms, 150ms, 300ms, 600ms, max 1500ms
      const delay = Math.min(80 * Math.pow(1.7, attempt), 1500);
      if (attempt % 5 === 0) {
        console.warn(`Backend initializing... Retrying in ${Math.round(delay)}ms (attempt ${attempt + 1})`);
      }
      setTimeout(() => this.init(attempt + 1), delay);
    }
  }

  setupEventStream() {
    apiClient.connectEvents(
      (eventData) => this.handleAgentEvent(eventData),
      (err) => console.warn("SSE connection status:", err)
    );
  }

  handleAgentEvent(eventObj) {
    if (!eventObj) return;
    const eventType = eventObj.event;
    const payload = eventObj.payload;

    console.log(`[Backend Event] ${eventType}`, payload);

    switch (eventType) {
      case 'scan_started':
        petVoiceEngine.trigger('diagnosing');
        break;

      case 'scan_completed':
        if (payload?.profile) {
          const tools = payload.profile.detected_tools || [];
          store.setMascotState('idle', petVoiceEngine.line('idle'));
        }
        break;

      case 'planning_started':
        petVoiceEngine.trigger('thinking', { query: payload?.goal || 'goal' });
        break;

      case 'recipe_matched':
        store.setMascotState('speaking', petVoiceEngine.line('thinking'));
        break;

      case 'plan_generated':
        petVoiceEngine.trigger('plan_ready', { stepCount: payload?.steps?.length || 5 });
        break;

      case 'safety_check_started':
        store.setMascotState('scanning', petVoiceEngine.line('diagnosing'));
        break;

      case 'safety_check_completed':
        if (payload?.report) {
          const rating = (payload.report.overall_rating || 'green').toLowerCase();
          if (rating === 'green') {
            sounds.play('verdict.green');
            store.setMascotState('green', petVoiceEngine.line('done'));
          } else if (rating === 'yellow') {
            petVoiceEngine.trigger('warning_held', { title: 'Held step' });
          } else if (rating === 'red') {
            sounds.play('verdict.red');
            store.setMascotState('red', petVoiceEngine.line('blocked'));
          }
        }
        break;

      case 'step_execution_started':
        sounds.play('task.start');
        store.setMode('hacker');
        petVoiceEngine.trigger('step_progress', { desc: payload?.description || payload?.step_id });

        store.setState(s => {
          const tasks = (s.activePlan?.tasks || []).map(t => {
            if (t.id === payload?.step_id) return { ...t, status: 'running' };
            return t;
          });
          return {
            ...s,
            activePlan: { ...s.activePlan, status: 'running', tasks },
            terminalLogs: [
              ...s.terminalLogs,
              { id: Date.now() + Math.random(), text: `> [${payload?.step_id}] ${payload?.description}`, type: 'info' }
            ]
          };
        });
        break;

      case 'step_execution_progress':
        if (payload?.bytes_downloaded) {
          store.setMascotState('downloading', petVoiceEngine.line('downloading'));
        }
        store.setState(s => ({
          ...s,
          terminalLogs: [
            ...s.terminalLogs,
            { id: Date.now() + Math.random(), text: `~ ${payload?.message || 'progress'}`, type: 'running' }
          ]
        }));
        break;

      case 'step_execution_completed':
        sounds.play('task.done');
        sounds.play('verdict.green');
        store.setMascotState('green', petVoiceEngine.line('done'));

        store.setState(s => {
          const tasks = (s.activePlan?.tasks || []).map(t => {
            if (t.id === payload?.step_id) return { ...t, status: 'done' };
            return t;
          });
          return {
            ...s,
            activePlan: { ...s.activePlan, tasks },
            terminalLogs: [
              ...s.terminalLogs,
              { id: Date.now() + Math.random(), text: `✓ [${payload?.step_id}] completed & verified cleanly`, type: 'green' }
            ]
          };
        });
        break;

      case 'step_execution_blocked':
        sounds.play('verdict.red');
        store.setMascotState('red', petVoiceEngine.line('blocked'));

        store.setState(s => {
          const tasks = (s.activePlan?.tasks || []).map(t => {
            if (t.id === payload?.step_id) return { ...t, status: 'blocked' };
            return t;
          });
          return {
            ...s,
            terminalLogs: [
              ...s.terminalLogs,
              { id: Date.now() + Math.random(), text: `⛔ BLOCKED: ${payload?.reason}`, type: 'error' }
            ]
          };
        });
        break;

      case 'step_approval_required':
        sounds.play('verdict.yellow');
        store.setMascotState('held-sign', petVoiceEngine.line('warning'));

        store.setState(s => {
          const tasks = (s.activePlan?.tasks || []).map(t => {
            if (t.id === payload?.step_id) return { ...t, status: 'held' };
            return t;
          });
          return {
            ...s,
            activePlan: { ...s.activePlan, tasks },
            terminalLogs: [
              ...s.terminalLogs,
              { id: Date.now() + Math.random(), text: `? [${payload?.step_id}] HELD: ${(payload?.reasons || []).join('; ')}`, type: 'yellow' }
            ]
          };
        });
        break;

      case 'verification_started':
        store.setState(s => ({
          ...s,
          terminalLogs: [
            ...s.terminalLogs,
            { id: Date.now() + Math.random(), text: `> running runtime verification for ${payload?.step_id}...`, type: 'info' }
          ]
        }));
        break;

      case 'verification_completed':
        store.setState(s => ({
          ...s,
          terminalLogs: [
            ...s.terminalLogs,
            { id: Date.now() + Math.random(), text: `✓ verification check passed: ${JSON.stringify(payload?.result?.command_run || 'OK')}`, type: 'green' }
          ]
        }));
        break;

      case 'transaction_recorded':
        this.refreshHistory();
        break;

      case 'rollback_started':
        sounds.play('rewind');
        store.setMascotState('rewind', petVoiceEngine.line('rewind'));
        break;

      case 'rollback_completed':
        if (payload?.success) {
          sounds.play('verdict.green');
          store.setMascotState('green', petVoiceEngine.line('done'));
        } else {
          sounds.play('error');
          store.setMascotState('error', petVoiceEngine.line('error'));
        }
        this.refreshHistory();
        break;

      default:
        break;
    }
  }

  submitGoal(goal) {
    const conversationId = store.getState().activeConversationId;
    const previous = this.chatQueues.get(conversationId) || Promise.resolve();
    const current = previous.catch(() => null).then(() => this.runChatTurn(goal, conversationId));
    this.chatQueues.set(conversationId, current);
    const clearQueue = () => {
      if (this.chatQueues.get(conversationId) === current) this.chatQueues.delete(conversationId);
    };
    current.then(clearQueue, clearQueue);
    return current;
  }

  stopChat(conversationId = store.getState().activeConversationId) {
    this.chatControllers.get(conversationId)?.abort();
  }

  async runChatTurn(goal, conversationId) {
    const activePet = store.getState().activePet || 'ghost';
    sounds.play('msg.send');
    if (store.getState().activeConversationId === conversationId) {
      store.updateConversationTitleFromMessage(goal);
    }
    petVoiceEngine.trigger('thinking', { query: goal });

    const userMsg = {
      id: `u_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
      sender: 'user',
      text: goal,
      time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    };
    store.appendConversationMessage(conversationId, userMsg);
    requestAnimationFrame(() => requestAnimationFrame(() => {
      const state = store.getState();
      if (state.activeConversationId !== conversationId || state.activePage !== 'chats') return;
      const scrollArea = document.querySelector('#main-view-container .workspace-scroll-area');
      if (scrollArea) scrollArea.scrollTop = scrollArea.scrollHeight;
    }));
    if (store.getState().activeConversationId === conversationId) {
      store.setState(s => ({
        ...s,
        terminalLogs: [
          ...(s.terminalLogs || []).slice(-100),
          { id: Date.now() + Math.random(), text: '> user inquiry received', type: 'info' }
        ]
      }));
    }

    const assistantId = `a_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;
    const assistant = {
      id: assistantId,
      sender: 'assistant',
      text: '',
      options: [],
      time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
      planAttached: false,
      isStreaming: true,
      pet: activePet,
    };
    const controller = new AbortController();
    this.chatControllers.set(conversationId, controller);
    store.appendConversationMessage(conversationId, assistant);
    if (store.getState().activeConversationId === conversationId) {
      store.setState({ isChatGenerating: true });
    }

    let reply = '';
    let pendingText = '';
    let frame = null;
    let completed = null;
    const flush = () => {
      frame = null;
      reply = pendingText;
      store.updateConversationMessage(conversationId, assistantId, { text: reply, isStreaming: true });
      window.dispatchEvent(new CustomEvent('setuppal:stream-delta', {
        detail: { conversationId, messageId: assistantId, text: reply }
      }));
    };
    const persistTimer = window.setInterval(() => {
      if (frame !== null) {
        cancelAnimationFrame(frame);
        flush();
      }
      store.persistConversations();
    }, 1000);

    try {
      if (!this.isBackendOnline) {
        store.setMascotState('scanning', petVoiceEngine.line('thinking'));
        const ready = await this.waitForConnection(3500);
        if (!ready) {
          const health = await apiClient.checkHealth(1000);
          if (health) this.isBackendOnline = true;
        }
      }

      const previousTurns = store.getConversationMessages(conversationId)
        .filter(message =>
          message.id !== assistantId &&
          message.id !== userMsg.id &&
          !message.isStreaming &&
          !message.stopped &&
          !message.failed
        );
      const history = previousTurns.slice(-60).map(message => ({
        role: message.sender === 'user' ? 'user' : 'assistant',
        content: (message.text || '').trim().slice(0, 1200)
      }));

      await apiClient.sendChatStream(goal, history, activePet, {
        signal: controller.signal,
        onEvent: (event) => {
          if (event.type === 'message.delta' && typeof event.text === 'string') {
            pendingText += event.text;
            if (frame === null) frame = requestAnimationFrame(flush);
          } else if (event.type === 'message.end') {
            completed = event;
          } else if (event.type === 'message.error') {
            throw new Error(event.message || 'The response stream failed.');
          }
        }
      });

      if (frame !== null) {
        cancelAnimationFrame(frame);
        flush();
      }
      if (!completed) throw new Error('The response stream ended before completion.');

      reply = completed.reply || reply;
      if (completed.context) setLastContext(completed.context);
      store.updateConversationMessage(conversationId, assistantId, {
        text: reply,
        options: [],
        isStreaming: false,
      });
      store.persistConversations();
      if (store.getState().activeConversationId === conversationId) {
        store.setState(s => ({
          ...s,
          isChatGenerating: false,
          chatMessages: s.chatMessages.map(message =>
            message.id === assistantId ? { ...message, text: reply, isStreaming: false } : message
          ),
          terminalLogs: [
            ...s.terminalLogs,
            { id: Date.now() + Math.random(), text: '> assistant response complete', type: 'green' }
          ]
        }));
      }
      sounds.play('msg.receive');
      petVoiceEngine.trigger('questions_asked');
      return { reply, context: completed.context || null };
    } catch (error) {
      if (frame !== null) {
        cancelAnimationFrame(frame);
        flush();
      }
      if (controller.signal.aborted) {
        store.updateConversationMessage(conversationId, assistantId, {
          text: reply,
          isStreaming: false,
          stopped: true,
        });
        store.persistConversations();
        if (store.getState().activeConversationId === conversationId) {
          store.setState(s => ({
            ...s,
            isChatGenerating: false,
            chatMessages: s.chatMessages.map(message =>
              message.id === assistantId ? { ...message, text: reply, isStreaming: false, stopped: true } : message
            )
          }));
        }
        store.setMascotState('idle', petVoiceEngine.line('stopped'));
        return { reply, stopped: true };
      }

      const errorText = error?.message || 'The response could not be completed.';
      const partialReply = reply ? `${reply}\n\n**Connection issue:** ${errorText}` : `**Error:** ${errorText}`;
      store.updateConversationMessage(conversationId, assistantId, {
        text: partialReply,
        isStreaming: false,
        failed: true,
      });
      store.persistConversations();
      if (store.getState().activeConversationId === conversationId) {
        store.setState(s => ({
          ...s,
          isChatGenerating: false,
          chatMessages: s.chatMessages.map(message =>
            message.id === assistantId ? { ...message, text: partialReply, isStreaming: false, failed: true } : message
          )
        }));
      }
      sounds.play('error');
      store.setMascotState('error', petVoiceEngine.line('error'));
      return null;
    } finally {
      window.clearInterval(persistTimer);
      if (this.chatControllers.get(conversationId) === controller) {
        this.chatControllers.delete(conversationId);
      }
    }
  }

  async approveAndRunPlan(trustedCategories = []) {
    sounds.play('ui.click');
    petVoiceEngine.trigger('plan_approve');
    store.setMode('hacker');

    if (!this.isBackendOnline) {
      await this.waitForConnection(3000);
    }

    try {
      if (!this.activeExecutionPlan) {
        const initialGoal = store.getState().activePlan?.title || "Web and TypeScript development stack";
        const data = await apiClient.createPlan(initialGoal);
        this.activeExecutionPlan = data.plan;
      }

      const result = await apiClient.executePlan({
        planId: this.activeExecutionPlan?.plan_id,
        dryRun: false,
        autoApproveGreen: true,
        allowYellow: false,
        customPlan: this.activeExecutionPlan,
        trustedCategories: trustedCategories
      });

      console.log("Plan execution finished:", result);

      // Celebration
      sounds.play('task.all_done');
      sounds.play('celebrate');
      petVoiceEngine.trigger('completed');

      try {
        confetti({
          particleCount: 85,
          spread: 65,
          origin: { y: 0.6 },
          colors: ['#39FF14', '#E6E0FF', '#E9B949', '#5CFF3A']
        });
      } catch (e) {}

      store.setState(s => ({
        ...s,
        activePlan: { ...s.activePlan, status: 'completed' },
        terminalLogs: [
          ...s.terminalLogs,
          { id: Date.now() + 99, text: '✓ ALL APPROVED TASKS EXECUTED AND VERIFIED CLEANLY', type: 'green' }
        ]
      }));

      this.refreshHistory();

      setTimeout(() => {
        store.setMode('chill');
        store.setMascotState('idle', petVoiceEngine.line('idle'));
      }, 4000);

      return result;
    } catch (err) {
      console.error("Plan execution failed:", err);
      sounds.play('error');
      store.setMode('chill');
      store.setMascotState('error', petVoiceEngine.line('error'));
      store.setState(s => ({
        ...s,
        chatMessages: [
          ...s.chatMessages,
          {
            id: 'a_' + Date.now(),
            sender: 'assistant',
            text: `**Error:** ${err.message}`,
            options: [],
            time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
            planAttached: false
          }
        ]
      }));
    }
  }

  async refreshHistory() {
    try {
      if (!this.isBackendOnline) {
        await this.waitForConnection(2000);
      }
      const data = await apiClient.getHistory();
      if (data && Array.isArray(data.transactions)) {
        const rolledBack = new Set(
          data.transactions.map(tx => tx.rolled_back_tx_id).filter(Boolean)
        );
        const transactions = data.transactions.map(tx => ({
          id: tx.tx_id,
          timestamp: tx.timestamp,
          goal: tx.goal || '',
          stepId: tx.step_id,
          summary: tx.action_summary,
          dryRun: !!tx.dry_run,
          changes: tx.changes_made || [],
          rolledBackTxId: tx.rolled_back_tx_id || null,
          rollbackAvailable: !!tx.rollback_data && !rolledBack.has(tx.tx_id),
          step: tx.step
        }));

        const downloads = transactions
          .filter(tx => !tx.dryRun && !tx.rolledBackTxId && tx.step?.action === 'download_artifact')
          .map(tx => {
            const p = tx.step.params || {};
            const dest = String(p.destination || p.id || '');
            return {
              id: tx.id,
              filename: dest.split(/[\\/]/).pop() || p.id,
              source: p.adapter || 'unknown',
              sha256: p.expected_sha256 || '',
              date: new Date(tx.timestamp).toLocaleString(),
              rolledBack: rolledBack.has(tx.id)
            };
          })
          .reverse();

        store.setState(s => ({
          ...s,
          downloads,
          history: { transactions, loaded: true, error: null }
        }));
      }
    } catch (err) {
      console.warn("Could not load history from backend:", err);
      store.setState(s => ({
        ...s,
        history: { ...s.history, loaded: true, error: err.message || 'backend unavailable' }
      }));
    }
  }

  async rewindLastChange(txId = null) {
    sounds.play('rewind');
    store.setMascotState('rewind', petVoiceEngine.line('rewind'));

    if (!this.isBackendOnline) {
      await this.waitForConnection(3000);
    }

    try {
      const res = await apiClient.undo(txId);
      if (res.status === 'rolled_back') {
        sounds.play('verdict.green');
        store.setMascotState('green', petVoiceEngine.line('done'));
      } else {
        sounds.play('error');
        store.setMascotState('error', petVoiceEngine.line('error'));
      }
      this.refreshHistory();
      return res;
    } catch (err) {
      console.error("Rewind failed:", err);
      sounds.play('error');
      store.setMascotState('error', petVoiceEngine.line('error'));
    }
  }
}

export const bridge = new BackendBridge();
