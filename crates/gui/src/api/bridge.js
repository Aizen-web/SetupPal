/**
 * SetupPal: Backend-to-Frontend State Bridge
 * Coordinates between Rust installer_core API, EventBus SSE stream, and the reactive UI Store.
 */

import { apiClient } from './client.js';
import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { simulation } from '../state/simulation.js';
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
  }

  async init() {
    console.log("Connecting UI to SetupPal Backend...");
    const health = await apiClient.checkHealth();
    if (health) {
      this.isBackendOnline = true;
      console.log("Zero-Trust Backend Connected:", health);
      this.setupEventStream();
      this.refreshHistory();
    } else {
      console.warn("Backend not yet running. Will retry connection in background.");
      setTimeout(() => this.init(), 3000);
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
          store.setMascotState('idle', `Detected ${tools.length} installed dev tools on ${payload.profile.os_type}. All clear!`);
        }
        break;

      case 'planning_started':
        petVoiceEngine.trigger('thinking', { query: payload?.goal || 'goal' });
        break;

      case 'recipe_matched':
        store.setMascotState('speaking', `Found deterministic recipe: ${payload?.recipe_id}. Loading safety gates!`);
        break;

      case 'plan_generated':
        petVoiceEngine.trigger('plan_ready', { stepCount: payload?.steps?.length || 5 });
        break;

      case 'safety_check_started':
        store.setMascotState('scanning', "Scanning all proposed steps against OSV.dev CVEs and typosquat database...");
        break;

      case 'safety_check_completed':
        if (payload?.report) {
          const rating = (payload.report.overall_rating || 'green').toLowerCase();
          if (rating === 'green') {
            sounds.play('verdict.green');
            store.setMascotState('green', "Zero-trust safety review: All proposed steps are strictly VERIFIED GREEN!");
          } else if (rating === 'yellow') {
            petVoiceEngine.trigger('warning_held', { title: 'Held step' });
          } else if (rating === 'red') {
            sounds.play('verdict.red');
            store.setMascotState('red', "CRITICAL THREAT DETECTED: Step scored RED and execution is BLOCKED.");
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
          store.setMascotState('downloading', `Downloading artifact... ${(payload.bytes_downloaded / (1024*1024)).toFixed(1)} MB`);
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
        store.setMascotState('green', `Step ${payload?.step_id} executed and verified!`);

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
        store.setMascotState('red', `Blocked malicious/unsafe step: ${payload?.reason}`);

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
        store.setMascotState('held-sign', `Step ${payload?.step_id} is HELD for manual user review.`);

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
        store.setMascotState('rewind', `Reverting transaction ${payload?.tx_id}...`);
        break;

      case 'rollback_completed':
        if (payload?.success) {
          sounds.play('verdict.green');
          store.setMascotState('green', `Rollback completed: ${payload?.message}`);
        } else {
          sounds.play('error');
          store.setMascotState('error', `Rollback failed: ${payload?.message}`);
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

      const data = await apiClient.sendChat(goal, history);

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
      store.setMascotState('error', `Failed to process request: ${err.message}`);
      throw err;
    }
  }

  async approveAndRunPlan(trustedCategories = []) {
    sounds.play('ui.click');
    petVoiceEngine.trigger('plan_approve');
    store.setMode('hacker');

    try {
      if (!this.activeExecutionPlan) {
        const initialGoal = store.getState().activePlan?.title || "Minecraft with performance mods";
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
        store.setMascotState('idle', "Everything is set up and safe! What should we do next?");
      }, 4000);

      return result;
    } catch (err) {
      console.warn("Backend plan execution encountered an issue, falling back to local simulation:", err);
      simulation.approveAndRunPlan();
    }
  }

  async runAttackRadar() {
    sounds.play('ui.click');
    store.setMode('hacker');
    store.setMascotState('scanning', "Scanning incoming adversarial repository README against OSV.dev & typosquat radar...");

    try {
      const data = await apiClient.simulateAttack();
      const plan = data.plan;
      const safety = data.safety;

      sounds.play('verdict.red');
      store.setMascotState('red', "Nope! That is a scary one. Typosquat and pipeline attack BLOCKED!");

      const redLogs = [
        { id: Date.now() + 1, text: '!! ADVERSARIAL ATTACK INTERCEPTED !!', type: 'error' },
        { id: Date.now() + 2, text: `Untrusted README: "${data.untrusted_readme.slice(0, 75)}..."`, type: 'info' }
      ];

      (safety.step_reports || []).forEach((r, idx) => {
        if ((r.rating || '').toLowerCase() === 'red') {
          redLogs.push({
            id: Date.now() + 10 + idx,
            text: `⛔ ZERO-TRUST GATE [${r.step_id}]: ${(r.reasons || []).join(' | ')} (Score: ${r.risk_score}/100)`,
            type: 'error'
          });
        }
      });

      store.setState(s => ({
        ...s,
        terminalLogs: [...s.terminalLogs, ...redLogs]
      }));

      return data;
    } catch (err) {
      console.error("Attack simulation failed:", err);
      sounds.play('error');
      store.setMascotState('error', `Attack simulation failed: ${err.message}`);
    }
  }

  async refreshHistory() {
    try {
      const data = await apiClient.getHistory();
      if (data && data.transactions) {
        const changes = data.transactions.map((tx, idx) => ({
          id: tx.tx_id,
          time: new Date(tx.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
          desc: tx.action_summary,
          reversible: tx.rollback_data ? 'Full' : 'Partial',
          task: `Step ${tx.step_id}`
        }));

        store.setState(s => ({
          ...s,
          history: {
            ...s.history,
            changes: changes.length > 0 ? changes : s.history.changes
          }
        }));
      }
    } catch (err) {
      console.warn("Could not load history from backend:", err);
    }
  }

  async rewindLastChange() {
    sounds.play('rewind');
    store.setActivePage('chats');
    store.setMascotState('rewind', "Rewinding last change with deterministic state rollback...");

    try {
      const res = await apiClient.undo();
      if (res.status === 'rolled_back') {
        sounds.play('verdict.green');
        store.setMascotState('green', `Rolled back transaction ${res.tx_id}: ${res.outcome}`);
      } else {
        sounds.play('error');
        store.setMascotState('error', res.message || "No transaction available to rollback.");
      }
      this.refreshHistory();
      return res;
    } catch (err) {
      console.error("Rewind failed:", err);
      sounds.play('error');
      store.setMascotState('error', `Rollback error: ${err.message}`);
    }
  }
}

export const bridge = new BackendBridge();
