/**
 * SetupPal: Pixel's reaction engine.
 *
 * Maps app events to a mascot animation state, a sound, and a short flavor line from
 * pixel-lines.js. Events carry no payload on purpose: Pixel never repeats user text.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { pixelLine } from './pixel-lines.js';

// event -> { line bank key, mascot animation state, optional sound }
const EVENTS = {
  welcome:         { line: 'welcome',      state: 'idle' },
  typing:          { line: 'typing',       state: 'listening' },
  thinking:        { line: 'thinking',     state: 'thinking' },
  diagnosing:      { line: 'scanning',     state: 'thinking',  sound: 'ui.click' },
  questions_asked: { line: 'questions',    state: 'listening', sound: 'msg.receive' },
  plan_ready:      { line: 'plan_ready',   state: 'idle',      sound: 'plan.ready' },
  plan_approve:    { line: 'plan_approve', state: 'working',   sound: 'ui.click' },
  step_progress:   { line: 'working',      state: 'working' },
  warning_held:    { line: 'warning',      state: 'warning',   sound: 'verdict.yellow' },
  blocked:         { line: 'blocked',      state: 'blocked',   sound: 'verdict.red' },
  completed:       { line: 'done',         state: 'success',   sound: 'task.done' },
  poke:            { line: 'poke',         state: 'success',   sound: 'task.done' },
  prompt_selected: { line: 'prompt',       state: 'thinking',  sound: 'ui.click' },
  error:           { line: 'error',        state: 'error',     sound: 'error' },
  rewind:          { line: 'rewind',       state: 'working' },
  downloading:     { line: 'downloading',  state: 'working' },
  idle:            { line: 'idle',         state: 'idle' },
  new_chat:        { line: 'new_chat',     state: 'success',   sound: 'ui.click' },
  switch_chat:     { line: 'switch_chat',  state: 'idle',      sound: 'ui.click' },
  stopped:         { line: 'stopped',      state: 'idle' },
};

class PixelVoice {
  /** Returns a flavor line without touching the UI. Accepts an event name or a line bank name. */
  line(event) {
    const def = EVENTS[event];
    return pixelLine(def ? def.line : event);
  }

  trigger(event) {
    const def = EVENTS[event] || EVENTS.welcome;
    if (def.sound) sounds.play(def.sound);
    store.setMascotState(def.state, pixelLine(def.line));
  }

  /** Mode switches: the line depends on the mode, not on anything the user typed. */
  modeChanged(mode) {
    const hacker = mode === 'hacker';
    store.setMascotState(hacker ? 'working' : 'idle', pixelLine(hacker ? 'hacker' : 'chill'));
  }
}

export const pixelVoice = new PixelVoice();
