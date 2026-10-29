/**
 * SetupPal: Pet Voiceline & Event Engine
 *
 * Maps app events to a mascot animation state, a sound, and a short flavor line.
 * Lines live in pet-lines.js and are intentionally vague and playful: they never
 * quote the user or report facts (that is the chat's job).
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { petLine } from './pet-lines.js';

// event -> { line bank key, mascot animation state, optional sound, optional mode-dependent key }
const EVENTS = {
  welcome:         { line: 'welcome',      state: 'idle' },
  typing:          { line: 'typing',       state: 'listening' },
  thinking:        { line: 'thinking',     state: 'thinking' },
  diagnosing:      { line: 'scanning',     state: 'scanning',  sound: 'ui.click' },
  questions_asked: { line: 'questions',    state: 'speaking',  sound: 'msg.receive' },
  plan_ready:      { line: 'plan_ready',   state: 'idle',      sound: 'plan.ready' },
  plan_approve:    { line: 'plan_approve', state: 'speaking',  sound: 'ui.click' },
  step_progress:   { line: 'working',      state: 'running' },
  warning_held:    { line: 'warning',      state: 'yellow',    sound: 'verdict.yellow' },
  blocked:         { line: 'blocked',      state: 'red',       sound: 'verdict.red' },
  completed:       { line: 'done',         state: 'celebrate', sound: 'task.done' },
  poke:            { line: 'poke',         state: 'celebrate', sound: 'task.done' },
  pet_change:      { line: 'welcome',      state: 'wave-hello', sound: 'ui.click' },
  prompt_selected: { line: 'prompt',       state: 'thinking',  sound: 'ui.click' },
  error:           { line: 'error',        state: 'error',     sound: 'error' },
  rewind:          { line: 'rewind',       state: 'rewind' },
  downloading:     { line: 'downloading',  state: 'downloading' },
  idle:            { line: 'idle',         state: 'idle' },
  new_chat:        { line: 'new_chat',     state: 'idle',      sound: 'ui.click' },
  switch_chat:     { line: 'switch_chat',  state: 'idle',      sound: 'ui.click' },
  stopped:         { line: 'stopped',      state: 'idle' },
};

export class PetVoiceEngine {
  getActivePet() {
    const key = store.getState().activePet || 'ghost';
    return ['ghost', 'cat', 'bunny'].includes(key) ? key : 'ghost';
  }

  /** Returns a flavor line for the active pet without touching the UI. */
  line(event) {
    const def = EVENTS[event];
    return petLine(this.getActivePet(), def ? def.line : 'idle');
  }

  trigger(event, payload = {}) {
    const pet = this.getActivePet();

    // Mode toggle: line and animation depend on the mode, not on user text.
    if (event === 'mode_toggle') {
      const hacker = payload.mode === 'hacker';
      sounds.play('ui.click');
      store.setMascotState(hacker ? 'speaking' : 'idle', petLine(pet, hacker ? 'hacker' : 'chill'));
      return;
    }

    const def = EVENTS[event] || EVENTS.welcome;
    if (def.sound) sounds.play(def.sound);
    store.setMascotState(def.state, petLine(pet, def.line));
  }
}

export const petVoiceEngine = new PetVoiceEngine();
