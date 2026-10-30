export const ANIMATIONS = [
  { id: 'idle', label: 'Cozy float', durationMs: 5000, loop: true, priority: 1, sound: 'ui.click' },
  { id: 'listening', label: 'Listening lean', durationMs: 3200, loop: true, priority: 2, sound: 'ui.click' },
  { id: 'thinking', label: 'Thinking dots', durationMs: 3000, loop: true, priority: 2, sound: 'ui.click' },
  { id: 'working', label: 'Busy bounce', durationMs: 2400, loop: true, priority: 2, sound: 'task.start' },
  { id: 'success', label: 'Happy pop', durationMs: 1800, loop: false, priority: 3, sound: 'celebrate' },
  { id: 'warning', label: 'Careful wobble', durationMs: 3200, loop: true, priority: 3, sound: 'verdict.yellow' },
  { id: 'blocked', label: 'Protective pulse', durationMs: 2800, loop: true, priority: 3, sound: 'verdict.red' },
  { id: 'error', label: 'Gentle oops', durationMs: 3400, loop: true, priority: 3, sound: 'error' },
];

const STATE_ALIASES = {
  blink: 'idle',
  speaking: 'listening',
  scanning: 'thinking',
  downloading: 'working',
  rewind: 'working',
  green: 'success',
  celebrate: 'success',
  'wave-hello': 'success',
  yellow: 'warning',
  'held-sign': 'warning',
  red: 'blocked',
  sleepy: 'idle',
  'peek-a-boo': 'idle',
  'glasses-on': 'idle',
  'glasses-off': 'idle',
  diagnosing: 'thinking',
};

const REGISTERED_STATES = new Set(ANIMATIONS.map(({ id }) => id));

export function normalizeMascotState(state) {
  const normalized = STATE_ALIASES[state] || state;
  return REGISTERED_STATES.has(normalized) ? normalized : 'idle';
}
