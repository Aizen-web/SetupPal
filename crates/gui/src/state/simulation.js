/**
 * SetupPal: Simulation Engine for UI Demo
 * Runs interactive execution flows, auto-switching between Chill and Hacker modes
 */

import { store } from './store.js';
import { sounds } from '../audio/sound-engine.js';
import { petVoiceEngine } from '../mascot/voicelines.js';
import confetti from 'canvas-confetti';

class SimulationEngine {
  constructor() {
    this.timer = null;
    this.isRunning = false;
  }

  approveAndRunPlan() {
    if (this.isRunning) return;
    this.isRunning = true;

    // 1. Initial approval reaction in Chill
    store.setMascotState('speaking', petVoiceEngine.line('plan_approve'));
    sounds.play('ui.click');

    setTimeout(() => {
      // 2. AUTO-ENTER HACKER MODE
      store.setMode('hacker');
      store.setMascotState('downloading', petVoiceEngine.line('downloading'));
      
      // Update plan status
      store.setState(s => {
        const tasks = s.activePlan.tasks.map(t => {
          if (t.id === 3) return { ...t, status: 'running' };
          return t;
        });
        return {
          ...s,
          activePlan: { ...s.activePlan, status: 'running', tasks },
          terminalLogs: [
            ...s.terminalLogs,
            { id: Date.now() + 1, text: '> execution started: strictly verified pipeline', type: 'info' },
            { id: Date.now() + 2, text: '~ downloading fabric-installer-1.0.1.exe (18.0 MB)...', type: 'running' }
          ]
        };
      });
      sounds.play('task.start');

      // 3. Download progresses (Simulate 2.5s download)
      setTimeout(() => {
        sounds.play('download.done');
        sounds.play('verdict.green');

        store.setMascotState('working', petVoiceEngine.line('working'));
        store.setState(s => {
          const tasks = s.activePlan.tasks.map(t => {
            if (t.id === 3) return { ...t, status: 'done' };
            if (t.id === 4) return { ...t, status: 'running' };
            return t;
          });
          return {
            ...s,
            activePlan: { ...s.activePlan, tasks },
            terminalLogs: [
              ...s.terminalLogs,
              { id: Date.now() + 3, text: '✓ fabric loader: sha256 9f86d081 matches official publisher', type: 'green' },
              { id: Date.now() + 4, text: '~ adding Sodium 0.5.11 from modrinth.com...', type: 'running' }
            ]
          };
        });

        // 4. Sodium installation & verification (1.8s)
        setTimeout(() => {
          sounds.play('task.done');
          store.setMascotState('green', petVoiceEngine.line('done'));

          store.setState(s => {
            const tasks = s.activePlan.tasks.map(t => {
              if (t.id === 4) return { ...t, status: 'done' };
              return t;
            });
            return {
              ...s,
              activePlan: { ...s.activePlan, tasks },
              terminalLogs: [
                ...s.terminalLogs,
                { id: Date.now() + 5, text: '✓ sodium: hash verified, installed to mods folder', type: 'green' },
                { id: Date.now() + 6, text: '? step 5 (FastLight Pro) held (yellow): brand new author, zero endorsements', type: 'yellow' }
              ]
            };
          });

          // 5. Yellow notice for held task
          setTimeout(() => {
            sounds.play('verdict.yellow');
            store.setMascotState('held-sign', petVoiceEngine.line('warning'));

            // 6. Celebration of approved tasks!
            setTimeout(() => {
              sounds.play('task.all_done');
              sounds.play('celebrate');
              store.setMascotState('celebrate', petVoiceEngine.line('done'));

              // Confetti burst
              try {
                confetti({
                  particleCount: 75,
                  spread: 60,
                  origin: { y: 0.6 },
                  colors: ['#39FF14', '#E6E0FF', '#E9B949', '#5CFF3A']
                });
              } catch (e) {}

              store.setState(s => ({
                ...s,
                activePlan: { ...s.activePlan, status: 'completed' },
                terminalLogs: [
                  ...s.terminalLogs,
                  { id: Date.now() + 7, text: '✓ ALL 4 APPROVED TASKS VERIFIED AND DONE', type: 'green' },
                  { id: Date.now() + 8, text: '> runtime test: minecraft client launch check OK', type: 'info' }
                ]
              }));

              // 7. Auto return to Chill Mode after dwell time (ui.md section 2.3)
              setTimeout(() => {
                store.setMode('chill');
                store.setMascotState('idle', petVoiceEngine.line('idle'));
                this.isRunning = false;
              }, 4000);

            }, 2000);
          }, 1800);
        }, 2000);
      }, 2500);
    }, 700);
  }

  simulateAttackRadar() {
    if (this.isRunning) return;
    this.isRunning = true;

    store.setMode('hacker');
    store.setMascotState('scanning', petVoiceEngine.line('diagnosing'));
    sounds.play('ui.click');

    setTimeout(() => {
      sounds.play('verdict.red');
      store.setMascotState('red', petVoiceEngine.line('blocked'));

      store.setState(s => ({
        ...s,
        terminalLogs: [
          ...s.terminalLogs,
          { id: Date.now() + 10, text: '!! ALERT: Evaluating package "reqwests" !!', type: 'error' },
          { id: Date.now() + 11, text: '✗ Levenshtein distance: 1 edit away from popular "requests" (94% match)', type: 'error' },
          { id: Date.now() + 12, text: '✗ OSV.dev & Heuristics: Zero downloads, newly registered author', type: 'error' },
          { id: Date.now() + 13, text: '⛔ ZERO-TRUST GATE: Execution BLOCKED with RED risk score 100/100', type: 'error' }
        ]
      }));

      setTimeout(() => {
        this.isRunning = false;
      }, 3000);
    }, 1800);
  }

  stopRun() {
    this.isRunning = false;
    store.setMode('chill');
    store.setMascotState('idle', petVoiceEngine.line('stopped'));
    sounds.play('mode.chill');
  }
}

export const simulation = new SimulationEngine();
