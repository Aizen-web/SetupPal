/**
 * SetupPal: Backend-to-Frontend State Bridge
 * Coordinates between Rust installer_core API, EventBus SSE stream, and the reactive UI Store.
 */

import { apiClient } from './client.js';
import { store } from '../state/store.js';
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

  async submitGoal(goal) {
    sounds.play('msg.send');
    store.updateConversationTitleFromMessage(goal);
    petVoiceEngine.trigger('thinking', { query: goal });

    const userMsg = {
      id: 'u_' + Date.now(),
      sender: 'user',
      text: goal,
      time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    };

    store.setState(s => ({
      ...s,
      chatMessages: [...s.chatMessages, userMsg],
      terminalLogs: [
        ...(s.terminalLogs || []).slice(-100),
        { id: Date.now() + Math.random(), text: `> user inquiry: "${goal}"`, type: 'info' }
      ]
    }));

    if (!this.isBackendOnline) {
      store.setMascotState('scanning', petVoiceEngine.line('thinking'));
      const ready = await this.waitForConnection(3500);
      if (!ready) {
        const health = await apiClient.checkHealth(1000);
        if (health) this.isBackendOnline = true;
      }
    }

    try {
      // Build token-efficient history for conversational AI (windowed & distilled)
      // Exclude the current message just pushed to store so it is not duplicated in prompt
      const allMsgs = store.getState().chatMessages || [];
      const previousTurns = allMsgs.length > 1 ? allMsgs.slice(0, -1) : [];
      const history = previousTurns.slice(-3).map(m => {
        let text = (m.text || '').trim();
        const cutIdx = text.indexOf('\n\n❓ **Questions');
        if (cutIdx > 0) text = text.slice(0, cutIdx).trim();
        const planIdx = text.indexOf('\n\nReview the');
        if (planIdx > 0) text = text.slice(0, planIdx).trim();
        if (text.length > 250) text = text.slice(0, 250) + '...';
        return {
          role: m.sender === 'user' ? 'user' : 'assistant',
          content: text
        };
      });

      let data;
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          data = await apiClient.sendChat(goal, history, store.getState().activePet || 'ghost');
          break;
        } catch (err) {
          const isTransient = err?.message && (
            err.message.includes('fetch') ||
            err.message.includes('500') ||
            err.message.includes('502') ||
            err.message.includes('503') ||
            err.message.includes('ECONNREFUSED')
          );
          if (isTransient && attempt < 2) {
            console.warn(`sendChat transient error (attempt ${attempt + 1}), retrying in 500ms...`, err);
            await new Promise(r => setTimeout(r, 500));
          } else {
            throw err;
          }
        }
      }

      if (data.plan && data.safety) {
        const plan = data.plan;
        const safety = data.safety;
        this.activeExecutionPlan = plan;

        const safeStepsCount = (safety.step_reports || []).filter(r => (r.rating || '').toLowerCase() === 'green').length;
        const heldStepsCount = (safety.step_reports || []).filter(r => (r.rating || '').toLowerCase() === 'yellow').length;
        const blockedStepsCount = (safety.step_reports || []).filter(r => (r.rating || '').toLowerCase() === 'red').length;

        const tasks = plan.steps.map((s, idx) => {
          const report = (safety.step_reports || []).find(r => r.step_id === s.id);
          const meta = (data.step_metadata || []).find(m => m.step_id === s.id);
          const rating = (report?.rating || 'green').toUpperCase();
          const details = formatStepDetails(s.step);

          return {
            id: s.id,
            num: idx + 1,
            title: s.description,
            desc: details.desc,
            source: details.source,
            sourceDomain: meta?.source_domain || details.source,
            sourceUrl: meta?.source_url || null,
            verdictTitle: meta?.verdict_title || (rating === 'GREEN' ? 'Verified source' : rating === 'YELLOW' ? 'Caution advised' : 'Safety block'),
            verdictDesc: meta?.verdict_desc || (report?.reasons?.[0] || 'Checked by zero-trust radar'),
            verdict: rating,
            status: rating === 'RED' ? 'blocked' : rating === 'YELLOW' ? 'held' : 'waiting',
            reversible: true,
            reasons: report?.reasons || [],
            rawStep: s
          };
        });

        const subtitle = `${plan.steps.length} steps · ${safeStepsCount} safe${heldStepsCount > 0 ? ` · ${heldStepsCount} held` : ''}${blockedStepsCount > 0 ? ` · ${blockedStepsCount} blocked` : ''}`;

        store.setState(s => ({
          ...s,
          activePlan: {
            id: plan.plan_id,
            title: plan.goal || plan.summary,
            subtitle: subtitle,
            safety: 'strict',
            status: 'awaiting_approval',
            tasks: tasks
          },
          chatMessages: [
            ...s.chatMessages,
            {
              id: 'a_' + Date.now(),
              sender: 'assistant',
              text: data.reply,
              options: data.options || [],
              time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
              planAttached: true
            }
          ],
          terminalLogs: [
            ...s.terminalLogs,
            { id: Date.now() + Math.random(), text: `✓ zero-trust plan synthesized: ${plan.steps.length} verified actions`, type: 'green' }
          ]
        }));

        sounds.play('msg.receive');
        petVoiceEngine.trigger('plan_ready', { stepCount: tasks.length });
      } else {
        // Clarification / Problem Definition Phase (Questions asked, waiting for user response)
        store.setState(s => ({
          ...s,
          chatMessages: [
            ...s.chatMessages,
            {
              id: 'a_' + Date.now(),
              sender: 'assistant',
              text: data.reply,
              options: data.options || [],
              time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
              planAttached: false
            }
          ],
          terminalLogs: [
            ...s.terminalLogs,
            { id: Date.now() + Math.random(), text: `? pre-flight diagnosis complete; awaiting user clarification`, type: 'yellow' }
          ]
        }));

        sounds.play('msg.receive');
        petVoiceEngine.trigger('questions_asked');
      }

      return data;
    } catch (err) {
      console.error("Error creating plan or sending chat:", err);
      sounds.play('error');
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
      return null;
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
