/**
 * SetupPal: Backend-to-Frontend State Bridge
 * Coordinates between Rust installer_core API, EventBus SSE stream, and the reactive UI Store.
 */

import { apiClient } from './client.js';
import { store } from '../state/store.js';
import { setLastContext, motionReduced, getSafetyLevel } from '../state/prefs.js';
import { migrateLegacyKeys } from '../state/key-migration.js';
import { getPrefs, setPrefs } from '../state/prefs.js';
import { refreshKeyStatus } from '../state/key-status.js';
import { sounds } from '../audio/sound-engine.js';
import { pixelVoice } from '../mascot/voicelines.js';
import confetti from 'canvas-confetti';
import { labelRunText, runSummaryText, scanLogEntry } from './run-log.js';

/** Readable card title: prefer package name over a raw winget id goal. */
function planTitle(plan) {
  const goal = String(plan?.goal || '').trim();
  const firstInstall = (plan?.steps || []).find(s => s.step?.action === 'install_package');
  const pkg = firstInstall?.step?.params?.package_id || '';
  if (plan?.source?.resolved || (pkg && /^[A-Za-z0-9_.+-]+\.[A-Za-z0-9_.+-]+$/.test(goal))) {
    const name = pkg.includes('.') ? pkg.split('.').slice(1).join('.') : (pkg || goal);
    return name || goal;
  }
  if (goal.length <= 48) return goal;
  return goal.slice(0, 45) + '…';
}

/** Readable install step label with version when known (R23). */
export function installStepLabel(pkg, eco, version, fallbackDescription) {
  const channel = eco === 'winget' ? 'winget'
    : eco === 'pypi' ? 'pip'
    : (eco || 'package');
  const name = (() => {
    if (eco === 'winget' && pkg && pkg.includes('.')) {
      const last = pkg.split('.').pop();
      return last || pkg;
    }
    return pkg || 'package';
  })();
  if (version && version !== 'latest') {
    return `Install ${name} ${version} (${channel})`;
  }
  if (fallbackDescription && /^Install /i.test(fallbackDescription)) {
    return fallbackDescription;
  }
  return `Install ${name} (${channel})`;
}

/** Map `/api/plan` (or remediation) JSON into the UI `activePlan` shape. */
export function toActivePlan(data, { verifiedManual = false } = {}) {
  const plan = data?.plan;
  const safety = data?.safety;
  const metaList = Array.isArray(data?.step_metadata) ? data.step_metadata : [];
  if (!plan?.steps?.length) return null;

  const stepReport = (id) =>
    (safety?.step_reports || []).find(r => r.step_id === id) || null;

  const subtitleParts = [];
  if (verifiedManual) {
    subtitleParts.push('SetupPal checked: this installs unattended through winget.');
  }
  if (plan.summary) subtitleParts.push(plan.summary);
  const notes = Array.isArray(safety?.notes) ? safety.notes.filter(Boolean) : [];
  if (notes.length) subtitleParts.push(notes[0]);

  return {
    planId: plan.plan_id,
    title: planTitle(plan),
    subtitle: subtitleParts.join(' '),
    status: 'ready',
    verifiedManual: Boolean(verifiedManual),
    needsProjectDir: plan.steps.some(s =>
      s.step?.action === 'install_package' && s.step?.params?.ecosystem === 'npm'),
    projectDir: null,
    tasks: plan.steps.map((s, i) => {
      const report = stepReport(s.id);
      const meta = metaList.find(m => m.step_id === s.id) || {};
      const rating = (report?.rating || 'yellow').toLowerCase();
      const verdict = rating.toUpperCase();
      const pkg = s.step?.params?.package_id;
      const eco = s.step?.params?.ecosystem;
      const version = s.step?.params?.version || meta.resolved_version || null;
      let title = s.description;
      if (s.step?.action === 'install_package' && pkg) {
        title = installStepLabel(pkg, eco, version, s.description);
      }
      return {
        id: s.id,
        num: i + 1,
        title,
        sourceDomain: meta.source_domain || '',
        sourceUrl: meta.source_url || '',
        verdict,
        verdictTitle: meta.verdict_title || '',
        verdictDesc: meta.verdict_desc || '',
        reasons: Array.isArray(report?.reasons) ? report.reasons : [],
        status: verdict === 'RED' ? 'blocked' : 'waiting',
        approved: verdict === 'GREEN',
      };
    }),
  };
}

/** Intent items must appear in the user's messages — drop assistant-only suggestions (R2). */
export function intentGroundedInUserText(intent, historyMessages = []) {
  const userBlob = (historyMessages || [])
    .filter(m => m && m.sender === 'user')
    .map(m => String(m.text || '').toLowerCase())
    .join('\n');
  if (!userBlob.trim()) return false;

  const tokens = [];
  if (intent?.goal) tokens.push(String(intent.goal));
  if (intent?.item) tokens.push(String(intent.item));
  if (Array.isArray(intent?.items)) tokens.push(...intent.items.map(String));
  if (!tokens.length) return false;

  return tokens.every(tok => {
    const t = tok.toLowerCase().trim();
    if (!t) return false;
    if (userBlob.includes(t)) return true;
    // Publisher.Package → check last segment
    const last = t.split('.').pop();
    if (last && last.length >= 2 && userBlob.includes(last)) return true;
    // Multi-word goal ("Fabric for Minecraft Java"): every meaningful word must come from the user.
    const words = t.split(/[^a-z0-9+#.]+/).filter(w => w && !GOAL_FILLER_WORDS.has(w));
    return words.length > 0 && words.every(w => userBlob.includes(w));
  });
}

const GOAL_FILLER_WORDS = new Set([
  'a', 'an', 'and', 'the', 'for', 'with', 'to', 'of', 'on', 'in', 'my', 'plus',
  'set', 'setup', 'up', 'install', 'installer', 'latest', 'version', 'app', 'mod', 'mods', 'modded', 'loader',
]);

function setPlanPending(conversationId, label) {
  store.setState(s => {
    if (!label) {
      return s.planPending?.conversationId === conversationId ? { ...s, planPending: null } : s;
    }
    return { ...s, planPending: { conversationId, label } };
  });
}

function conversationSummary(messages, limit = 1400) {
  const lines = (messages || [])
    .filter(m => m && !m.isStreaming && !m.failed)
    .slice(-8)
    .map(m => {
      const role = m.sender === 'user' ? 'User' : 'Assistant';
      const text = String(m.text || '').replace(/\s+/g, ' ').trim().slice(0, 220);
      return `${role}: ${text}`;
    });
  let out = lines.join('\n');
  if (out.length > limit) out = out.slice(-limit);
  return out;
}

function escChat(text) {
  return String(text || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

/** One assistant chat line built from the run summary (R15) — not from the model. */
function buildRunChatSummary(summary) {
  const results = Array.isArray(summary?.step_results) ? summary.step_results : [];
  const lines = ['**Run finished.**'];
  for (const r of results) {
    const name = r.description || r.step_id || 'step';
    const st = String(r.status || '').toLowerCase();
    if (st === 'skipped') lines.push(`- Skipped: ${name}`);
    else if (st === 'failed') lines.push(`- Failed: ${name} — ${r.error || 'see the run log'}`);
    else if (r.executed && !r.error) {
      const already = (r.receipt?.logs || []).some(l => /already installed/i.test(String(l)));
      lines.push(already ? `- Already installed: ${name}` : `- Installed: ${name}`);
    }
  }
  if (lines.length === 1) lines.push('- Nothing new was installed.');
  return lines.join('\n');
}

export class BackendBridge {
  constructor() {
    this.isBackendOnline = false;
    this.activeExecutionPlan = null;
    this.activeRunId = null;
    /** True while the current run is a dry run (set from step_execution_started). */
    this.runDryRun = false;
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
      this.syncKeys();
    } else {
      // Rapid retry with progressive backoff: 80ms, 150ms, 300ms, 600ms, max 1500ms
      const delay = Math.min(80 * Math.pow(1.7, attempt), 1500);
      if (attempt % 5 === 0) {
        console.warn(`Backend initializing... Retrying in ${Math.round(delay)}ms (attempt ${attempt + 1})`);
      }
      setTimeout(() => this.init(attempt + 1), delay);
    }
  }

  /** Moves any old localStorage keys into the OS keychain, then loads key status from the backend. */
  async syncKeys() {
    try {
      await migrateLegacyKeys((name, value) => apiClient.saveKey(name, value));
      await refreshKeyStatus();
      await this.syncModelPreference();
      await apiClient.checkHealth();
    } catch (err) {
      console.warn('Key sync failed:', err?.message || err);
    }
  }

  /**
   * Keeps the Settings model selector aligned with the backend selection.
   * Backend file / SETUPPAL_MODEL is the inference source of truth; prefs mirror it.
   * If prefs differ and the backend is still on the default, push the saved UI choice once.
   */
  async syncModelPreference() {
    const prefs = getPrefs();
    const health = apiClient.lastHealth;
    const backendId = health?.model_id;
    if (backendId && prefs.model === backendId) return;
    if (prefs.model && prefs.model !== 'super' && (!backendId || backendId === 'super')) {
      try {
        await apiClient.setModel(prefs.model);
        return;
      } catch (err) {
        console.warn('Could not apply saved model preference:', err?.message || err);
      }
    }
    if (backendId) setPrefs({ model: backendId });
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
        pixelVoice.trigger('diagnosing');
        break;

      case 'scan_completed':
        if (payload?.profile) {
          // The backend scans on every chat turn; log it once per conversation unless facts change.
          const entry = scanLogEntry(store.getState().terminalLogs, payload.profile);
          if (entry) {
            store.setState(s => ({ ...s, terminalLogs: [...s.terminalLogs, entry] }));
          }
          store.setMascotState('idle', pixelVoice.line('idle'));
        }
        break;

      case 'planning_started':
        pixelVoice.trigger('thinking');
        break;

      case 'recipe_matched':
        store.setMascotState('speaking', pixelVoice.line('thinking'));
        break;

      case 'plan_generated': {
        const stepCount = payload?.plan?.steps?.length ?? 0;
        pixelVoice.trigger('plan_ready');
        if (stepCount) {
          store.setState(s => ({
            ...s,
            terminalLogs: [
              ...s.terminalLogs,
              {
                id: Date.now() + Math.random(),
                text: `~ plan ready with ${stepCount} step${stepCount === 1 ? '' : 's'}`,
                type: 'info',
              },
            ],
          }));
        }
        break;
      }

      case 'safety_check_started':
        store.setMascotState('scanning', pixelVoice.line('diagnosing'));
        break;

      case 'safety_check_completed':
        if (payload?.report) {
          const rating = (payload.report.overall_rating || 'green').toLowerCase();
          if (rating === 'green') {
            sounds.play('verdict.green');
            store.setMascotState('green', pixelVoice.line('done'));
          } else if (rating === 'yellow') {
            pixelVoice.trigger('warning_held');
          } else if (rating === 'red') {
            sounds.play('verdict.red');
            store.setMascotState('red', pixelVoice.line('blocked'));
          }
        }
        break;

      case 'step_execution_started':
        this.runDryRun = Boolean(payload?.dry_run);
        sounds.play('task.start');
        store.beginRun();
        pixelVoice.trigger('step_progress');

        store.setState(s => {
          const tasks = (s.activePlan?.tasks || []).map(t => {
            if (t.id === payload?.step_id) return { ...t, status: 'running' };
            return t;
          });
          return {
            ...s,
            activePlan: { ...s.activePlan, status: 'running', tasks, dryRun: this.runDryRun },
            terminalLogs: [
              ...s.terminalLogs,
              { id: Date.now() + Math.random(), text: labelRunText(`> [${payload?.step_id}] ${payload?.description}`, this.runDryRun), type: 'info' }
            ]
          };
        });
        break;

      case 'step_execution_progress':
        if (payload?.bytes_downloaded) {
          store.setMascotState('downloading', pixelVoice.line('downloading'));
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
        store.setMascotState('green', pixelVoice.line('done'));

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
              { id: Date.now() + Math.random(), text: (payload?.receipt?.dry_run ?? this.runDryRun)
                ? labelRunText(`✓ [${payload?.step_id}] simulated only: nothing was installed or changed`, true)
                : `✓ [${payload?.step_id}] completed & verified cleanly`, type: 'green' }
            ]
          };
        });
        break;

      case 'step_execution_blocked':
        sounds.play('verdict.red');
        store.setMascotState('red', pixelVoice.line('blocked'));

        store.setState(s => {
          const tasks = (s.activePlan?.tasks || []).map(t => {
            if (t.id === payload?.step_id) return { ...t, status: 'blocked' };
            return t;
          });
          return {
            ...s,
            activePlan: s.activePlan ? { ...s.activePlan, tasks } : s.activePlan,
            terminalLogs: [
              ...s.terminalLogs,
              { id: Date.now() + Math.random(), text: labelRunText(`⛔ BLOCKED: ${payload?.reason}`, this.runDryRun), type: 'error' }
            ]
          };
        });
        break;

      case 'step_execution_failed':
        sounds.play('error');
        store.setMascotState('error', pixelVoice.line('error'));
        store.setState(s => {
          const tasks = (s.activePlan?.tasks || []).map(t => {
            if (t.id === payload?.step_id) return { ...t, status: 'failed' };
            return t;
          });
          return {
            ...s,
            activePlan: s.activePlan ? { ...s.activePlan, tasks, status: 'failed' } : s.activePlan,
            terminalLogs: [
              ...s.terminalLogs,
              { id: Date.now() + Math.random(), text: labelRunText(`✗ [${payload?.step_id}] ${payload?.error || 'failed'}`, this.runDryRun), type: 'error' }
            ]
          };
        });
        break;

      case 'execution_cancelled':
        store.setState(s => {
          const tasks = (s.activePlan?.tasks || []).map(t => {
            if (t.id === payload?.step_id || t.status === 'running') {
              return { ...t, status: 'cancelled' };
            }
            return t;
          });
          return {
            ...s,
            activePlan: s.activePlan ? { ...s.activePlan, tasks, status: 'cancelled' } : s.activePlan,
            terminalLogs: [
              ...s.terminalLogs,
              { id: Date.now() + Math.random(), text: `■ cancelled at ${payload?.step_id || 'step'}`, type: 'yellow' }
            ]
          };
        });
        store.endRun();
        store.setMascotState('idle', pixelVoice.line('stopped'));
        break;

      case 'self_heal_triggered':
        store.setMascotState('diagnosing', pixelVoice.line('diagnosing'));
        store.setState(s => ({
          ...s,
          terminalLogs: [
            ...s.terminalLogs,
            {
              id: Date.now() + Math.random(),
              text: `~ self-heal: diagnosing ${payload?.failed_step_id || 'failed step'} (attempt ${payload?.attempt || 1})`,
              type: 'info',
            },
          ],
        }));
        break;

      case 'self_heal_plan_proposed':
        store.setState(s => ({
          ...s,
          terminalLogs: [
            ...s.terminalLogs,
            {
              id: Date.now() + Math.random(),
              text: `~ self-heal: remediation plan proposed (${payload?.remediation_plan?.steps?.length || 0} steps)`,
              type: 'info',
            },
          ],
        }));
        break;

      case 'step_approval_required':
        sounds.play('verdict.yellow');
        store.setMascotState('held-sign', pixelVoice.line('warning'));

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

      case 'verification_completed': {
        const passed = payload?.result?.passed !== false;
        const cmd = payload?.result?.command_run || 'check';
        store.setState(s => ({
          ...s,
          terminalLogs: [
            ...s.terminalLogs,
            {
              id: Date.now() + Math.random(),
              text: passed
                ? `✓ verification passed: ${cmd}`
                : `✗ verification failed: ${cmd} — ${payload?.result?.stderr || ''}`.trim(),
              type: passed ? 'green' : 'error',
            },
          ],
        }));
        break;
      }

      case 'transaction_recorded':
        this.refreshHistory();
        break;

      case 'rollback_started':
        sounds.play('rewind');
        store.setMascotState('rewind', pixelVoice.line('rewind'));
        break;

      case 'rollback_completed':
        if (payload?.success) {
          sounds.play('verdict.green');
          store.setMascotState('green', pixelVoice.line('done'));
        } else {
          sounds.play('error');
          store.setMascotState('error', pixelVoice.line('error'));
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

  async cancelRun() {
    if (!this.activeRunId) {
      store.endRun();
      return;
    }
    try {
      await apiClient.cancelExecute(this.activeRunId);
    } catch (err) {
      console.warn('Cancel request failed:', err);
      store.endRun();
    }
  }

  async runChatTurn(goal, conversationId) {
    sounds.play('msg.send');
    if (store.getState().activeConversationId === conversationId) {
      store.updateConversationTitleFromMessage(goal);
    }
    pixelVoice.trigger('thinking');

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
    const assistantId = `a_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;
    const assistant = {
      id: assistantId,
      sender: 'assistant',
      text: '',
      options: [],
      time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
      planAttached: false,
      isStreaming: true,
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
    let intent = null;
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
        store.setMascotState('scanning', pixelVoice.line('thinking'));
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

      await apiClient.sendChatStream(goal, history, {
        signal: controller.signal,
        onEvent: (event) => {
          if (event.type === 'message.delta' && typeof event.text === 'string') {
            pendingText += event.text;
            if (frame === null) frame = requestAnimationFrame(flush);
          } else if (event.type === 'plan.pending') {
            setPlanPending(conversationId, 'Working out what to install…');
          } else if (event.type === 'message.intent') {
            intent = event.intent || null;
            console.log('[SetupPal] message.intent', intent);
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
        intent: intent || null,
      });
      store.persistConversations();
      if (store.getState().activeConversationId === conversationId) {
        store.setState(s => ({
          ...s,
          isChatGenerating: false,
          chatMessages: s.chatMessages.map(message =>
            message.id === assistantId
              ? { ...message, text: reply, isStreaming: false, intent: intent || null }
              : message
          ),
        }));
      }
      sounds.play('msg.receive');
      pixelVoice.trigger('questions_asked');

      if (intent) {
        await this.handleIntent(conversationId, intent, store.getConversationMessages(conversationId));
      } else {
        setPlanPending(conversationId, null);
      }

      return { reply, context: completed.context || null, intent };
    } catch (error) {
      setPlanPending(conversationId, null);
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
        store.setMascotState('idle', pixelVoice.line('stopped'));
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
      store.setMascotState('error', pixelVoice.line('error'));
      return null;
    } finally {
      window.clearInterval(persistTimer);
      if (this.chatControllers.get(conversationId) === controller) {
        this.chatControllers.delete(conversationId);
      }
    }
  }

  async handleIntent(conversationId, intent, historyMessages = []) {
    try {
      await this.buildPlanFromIntent(conversationId, intent, historyMessages);
    } finally {
      setPlanPending(conversationId, null);
    }
  }

  async buildPlanFromIntent(conversationId, intent, historyMessages) {
    if (!intent?.kind) return;

    // The model's "manual" verdict is only a claim: the backend checks the item against winget, so
    // an app that installs silently still gets a plan even when the model called it manual-only.
    const verifyManual = intent.kind === 'manual';
    const goal = verifyManual
      ? intent.item
      : intent.kind === 'install'
        ? (intent.goal || (Array.isArray(intent.items) ? intent.items.join(', ') : null))
        : null;
    if (!goal) return;

    // Drop intents that name something only the assistant suggested (never the user).
    if (!intentGroundedInUserText(intent, historyMessages)) {
      console.warn('[SetupPal] Ignoring intent not grounded in user messages', intent);
      return;
    }

    // Broad "manual" with no specific item stays a chat question — don't open a guide.
    if (verifyManual && !String(intent.item || '').trim()) return;

    const showModelGuide = () => {
      store.setState(s => {
        if (s.activeConversationId !== conversationId) return s;
        return {
          ...s,
          activePlan: null,
          manualGuide: {
            item: intent.item,
            reason: intent.reason,
            alternatives: Array.isArray(intent.alternatives) ? intent.alternatives : [],
          },
        };
      });
      store.persistConversations();
    };

    // Pending card stays up while packages resolve and the safety checks run (R14).
    setPlanPending(conversationId, 'Working out what to install…');

    const clearStatus = (extra = {}) => {
      store.setState(s => ({
        ...s,
        ...extra,
        planPending: s.planPending?.conversationId === conversationId ? null : s.planPending,
      }));
    };

    try {
      if (!this.isBackendOnline) await this.waitForConnection(3000);
      setPlanPending(conversationId, 'Checking packages and running safety checks…');
      const data = await apiClient.createPlan(goal, {
        context: conversationSummary(historyMessages),
        verifyManual,
      });

      if (data?.kind === 'no_match') {
        clearStatus();
        showModelGuide();
        return;
      }

      if (data?.kind === 'broad') {
        // Broad requests stay in chat — no manual card (R18).
        clearStatus({ activePlan: null, manualGuide: null, packageConfirm: null });
        return;
      }

      if (data?.kind === 'confirm' && data?.suggestion) {
        clearStatus({
          activePlan: null,
          manualGuide: null,
          packageConfirm: {
            typed: data.typed || '',
            suggestion: data.suggestion,
            ecosystem: data.ecosystem || 'pypi',
            conversationId,
          },
        });
        store.persistConversations();
        return;
      }

      if (data?.kind === 'manual' || (data?.item && data?.reason && !data?.plan)) {
        clearStatus({
          activePlan: null,
          packageConfirm: null,
          manualGuide: {
            item: data.item,
            reason: data.detail || data.reason,
            alternatives: data.alternatives || [],
            steps: data.steps || [],
          },
        });
        store.persistConversations();
        return;
      }

      const activePlan = toActivePlan(data, { verifiedManual: verifyManual && Boolean(data?.plan) });
      if (!activePlan) {
        clearStatus();
        return;
      }
      this.activeExecutionPlan = data.plan;
      clearStatus({
        activePlan,
        manualGuide: null,
        packageConfirm: null,
        tasksOpen: true,
        scrollToPlan: true,
        // Prior plan is replaced; chat keeps one live card (R2 / R19).
        replacedPlanNote: store.getState().activePlan
          ? `Previous plan (${store.getState().activePlan.title}) was replaced.`
          : null,
      });
      store.persistConversations();
    } catch (err) {
      console.error('Plan from intent failed:', err);
      clearStatus();
      if (verifyManual) {
        showModelGuide();
        return;
      }
      store.setState(s => ({
        ...s,
        chatMessages: [
          ...s.chatMessages,
          {
            id: 'a_' + Date.now(),
            sender: 'assistant',
            text: `**Could not build a plan:** ${err.message}`,
            options: [],
            time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
            planAttached: false,
          },
        ],
      }));
    }
  }

  /**
   * @param {object} opts
   * @param {string[]} [opts.trustedCategories]
   * @param {string[]} [opts.approvedStepIds]
   * @param {string[]} [opts.allowYellowSteps]
   * @param {string|null} [opts.startAtStepId]
   */
  async approveAndRunPlan({
    trustedCategories = [],
    approvedStepIds = null,
    allowYellowSteps = null,
    startAtStepId = null,
    dryRun = false,
  } = {}) {
    sounds.play('ui.click');
    pixelVoice.trigger('plan_approve');
    store.beginRun();

    if (!this.isBackendOnline) {
      await this.waitForConnection(3000);
    }

    const active = store.getState().activePlan;
    const planId = active?.planId || this.activeExecutionPlan?.plan_id;
    if (!planId) {
      store.endRun();
      store.setMascotState('error', pixelVoice.line('error'));
      store.setState(s => ({
        ...s,
        chatMessages: [
          ...s.chatMessages,
          {
            id: 'a_' + Date.now(),
            sender: 'assistant',
            text: '**Error:** No plan is ready to run. Ask SetupPal to set something up first.',
            options: [],
            time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
            planAttached: false,
          },
        ],
      }));
      return null;
    }

    const tasks = active?.tasks || [];
    const greenIds = approvedStepIds
      ?? tasks.filter(t => t.verdict === 'GREEN' && t.approved !== false).map(t => t.id);
    const yellowIds = allowYellowSteps
      ?? tasks.filter(t => t.verdict === 'YELLOW' && t.approved).map(t => t.id);

    const safety = getSafetyLevel();
    const autoApproveGreen = safety !== 'strict';
    const runId = (typeof crypto !== 'undefined' && crypto.randomUUID)
      ? crypto.randomUUID()
      : `run-${Date.now()}`;
    this.activeRunId = runId;

    try {
      const result = await apiClient.executePlan({
        runId,
        planId,
        dryRun,
        autoApproveGreen,
        approvedStepIds: greenIds,
        allowYellowSteps: yellowIds,
        startAtStepId,
        projectDir: active?.projectDir || null,
        trustedCategories,
      });

      console.log('Plan execution finished:', result);
      this.activeRunId = null;

      const summary = result?.summary;
      const cancelled = Boolean(summary?.cancelled);
      const halted = Boolean(summary?.halted_at_step_id);
      const passed = Boolean(summary?.overall_passed);

      if (result?.remediation?.plan && !result.remediation.auto_ran) {
        const remPlan = toActivePlan(result.remediation);
        if (remPlan) {
          this.activeExecutionPlan = result.remediation.plan;
          store.setState(s => ({
            ...s,
            activePlan: { ...remPlan, status: 'ready' },
            terminalLogs: [
              ...s.terminalLogs,
              {
                id: Date.now() + Math.random(),
                text: result.remediation.blocked
                  ? '~ remediation scored RED and will not run'
                  : '~ remediation plan ready — review and approve to continue',
                type: result.remediation.blocked ? 'error' : 'yellow',
              },
            ],
          }));
          store.persistConversations();
        }
      }

      if (cancelled) {
        store.setState(s => ({
          ...s,
          activePlan: s.activePlan ? { ...s.activePlan, status: 'cancelled' } : s.activePlan,
        }));
        store.endRun();
        store.setMascotState('idle', pixelVoice.line('stopped'));
        this.refreshHistory();
        return result;
      }

      if (halted) {
        const heldId = summary.halted_at_step_id;
        store.setState(s => {
          const tasksNext = (s.activePlan?.tasks || []).map(t => {
            if (t.id === heldId) return { ...t, status: 'held' };
            return t;
          });
          return {
            ...s,
            activePlan: s.activePlan
              ? { ...s.activePlan, status: 'ready', tasks: tasksNext, haltedAt: heldId }
              : s.activePlan,
          };
        });
        store.endRun();
        pixelVoice.trigger('warning_held');
        this.refreshHistory();
        return result;
      }

      const ranDry = Boolean(result?.dry_run ?? summary?.dry_run);
      if (passed && ranDry) {
        // A simulation: say so plainly and skip the celebration.
        sounds.play('task.done');
        const line = runSummaryText({ dryRun: true, passed: true, headline: result?.headline });
        store.setState(s => ({
          ...s,
          activePlan: s.activePlan ? { ...s.activePlan, status: 'completed', dryRun: true } : s.activePlan,
          terminalLogs: [...s.terminalLogs, { id: Date.now() + 99, text: line, type: 'yellow' }],
          chatMessages: [
            ...s.chatMessages,
            {
              id: 'a_' + Date.now(),
              sender: 'assistant',
              text: `**Dry run only.** ${escChat(result?.headline || line)}`,
              options: [],
              time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
              planAttached: false,
            },
          ],
        }));
        store.endRun();
        store.setMascotState('idle', pixelVoice.line('idle'));
        this.refreshHistory();
        return result;
      }

      if (passed) {
        sounds.play('task.all_done');
        sounds.play('celebrate');
        pixelVoice.trigger('completed');
        if (!motionReduced()) {
          try {
            confetti({
              particleCount: 85,
              spread: 65,
              origin: { y: 0.6 },
              colors: ['#39FF14', '#E6E0FF', '#E9B949', '#5CFF3A'],
            });
          } catch (e) { /* ignore */ }
        }
        const chatSummary = buildRunChatSummary(summary);
        store.setState(s => ({
          ...s,
          activePlan: { ...s.activePlan, status: 'completed' },
          terminalLogs: [
            ...s.terminalLogs,
            { id: Date.now() + 99, text: '✓ ALL APPROVED TASKS EXECUTED AND VERIFIED CLEANLY', type: 'green' },
          ],
          chatMessages: [
            ...s.chatMessages,
            {
              id: 'a_' + Date.now(),
              sender: 'assistant',
              text: chatSummary,
              options: [],
              time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
              planAttached: false,
            },
          ],
        }));
        this.refreshHistory();
        setTimeout(() => {
          store.endRun();
          store.setMascotState('idle', pixelVoice.line('idle'));
        }, 4000);
        return result;
      }

      // Failed overall
      const failed = (summary?.step_results || []).find(r => r.status === 'failed' || r.error);
      const stderr = String(failed?.error || failed?.verification?.stderr || 'A step failed.')
        .slice(0, 600);
      sounds.play('error');
      store.endRun();
      store.setMascotState('error', pixelVoice.line('error'));
      store.setState(s => ({
        ...s,
        activePlan: s.activePlan ? { ...s.activePlan, status: 'failed' } : s.activePlan,
        chatMessages: [
          ...s.chatMessages,
          {
            id: 'a_' + Date.now(),
            sender: 'assistant',
            text: `**Step failed:** ${escChat(stderr)}`,
            options: [],
            time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
            planAttached: false,
          },
        ],
        terminalLogs: [
          ...s.terminalLogs,
          { id: Date.now() + Math.random(), text: `✗ ${stderr}`, type: 'error' },
        ],
      }));
      this.refreshHistory();
      return result;
    } catch (err) {
      console.error('Plan execution failed:', err);
      this.activeRunId = null;
      sounds.play('error');
      store.endRun();
      store.setMascotState('error', pixelVoice.line('error'));
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
            planAttached: false,
          },
        ],
      }));
      return null;
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
    store.setMascotState('rewind', pixelVoice.line('rewind'));

    if (!this.isBackendOnline) {
      await this.waitForConnection(3000);
    }

    try {
      const res = await apiClient.undo(txId);
      if (res.status === 'rolled_back') {
        sounds.play('verdict.green');
        store.setMascotState('green', pixelVoice.line('done'));
      } else {
        sounds.play('error');
        store.setMascotState('error', pixelVoice.line('error'));
      }
      this.refreshHistory();
      return res;
    } catch (err) {
      console.error("Rewind failed:", err);
      sounds.play('error');
      store.setMascotState('error', pixelVoice.line('error'));
      return { status: 'error', message: err?.message || 'The backend did not respond.' };
    }
  }
}

export const bridge = new BackendBridge();
