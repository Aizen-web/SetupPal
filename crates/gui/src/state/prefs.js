/**
 * SetupPal: persisted preferences + last-request context snapshot.
 * Kept tiny and separate from the main store so the API layer can read it anywhere.
 */

const KEY = 'setuppal_chat_prefs';
const UI_KEY = 'setuppal_ui_settings';

export const REPLY_LENGTHS = {
  short: { label: 'Short', tokens: 2400 },
  normal: { label: 'Normal', tokens: 3200 },
  long: { label: 'Long', tokens: 4400 },
};

/** Allowlisted Nemotron models (must match backend `NemotronModel`). */
export const NEMOTRON_MODELS = {
  lightning: {
    id: 'lightning',
    label: 'Nemotron 3.5 Lightning',
    description: 'Default. Fast, everyday installation planning',
  },
  super: {
    id: 'super',
    label: 'Nemotron 3 Super',
    description: 'Balanced reasoning and planning',
  },
  ultra: {
    id: 'ultra',
    label: 'Nemotron 3 Ultra',
    description: 'Advanced reasoning for complicated, multi-stage setups',
  },
};

export const DEFAULT_PREFS = {
  memoryTurns: 6, // recent messages sent word for word; older ones become compact notes
  replyLength: 'normal',
  includePcFacts: true,
  /** Allowlisted slug only — never an arbitrary model ID. Default: Lightning. */
  model: 'lightning',
};

function readJson(key) {
  try {
    const raw = JSON.parse(localStorage.getItem(key) || '{}');
    return raw && typeof raw === 'object' ? raw : {};
  } catch {
    return {};
  }
}

function sanitizePrefs(raw) {
  const next = { ...DEFAULT_PREFS, ...raw };
  next.memoryTurns = Number(next.memoryTurns) || DEFAULT_PREFS.memoryTurns;
  if (!REPLY_LENGTHS[next.replyLength]) next.replyLength = DEFAULT_PREFS.replyLength;
  next.includePcFacts = Boolean(next.includePcFacts);
  if (!NEMOTRON_MODELS[next.model]) next.model = DEFAULT_PREFS.model;
  return next;
}

export function getPrefs() {
  return sanitizePrefs(readJson(KEY));
}

export function setPrefs(patch) {
  const next = sanitizePrefs({ ...getPrefs(), ...patch });
  localStorage.setItem(KEY, JSON.stringify(next));
  return next;
}

export function resetPrefs() {
  localStorage.removeItem(KEY);
  return { ...DEFAULT_PREFS };
}

/** Fields the backend expects on /api/chat/stream. */
export function chatPrefsPayload() {
  const p = getPrefs();
  return {
    memory_turns: Number(p.memoryTurns) || DEFAULT_PREFS.memoryTurns,
    max_reply_tokens: (REPLY_LENGTHS[p.replyLength] || REPLY_LENGTHS.normal).tokens,
    include_pc_facts: Boolean(p.includePcFacts),
  };
}

/* ------------------------------------------------------------------ */
/* UI settings (Settings page)                                         */
/* ------------------------------------------------------------------ */

export const SAFETY_LEVELS = {
  strict: { label: 'Strict', hint: 'Every step waits for your approval. YELLOW steps are always held.' },
  balanced: { label: 'Balanced', hint: 'Safe, hash-verified steps can run after you approve the plan. YELLOW steps are held.' },
  relaxed: { label: 'Relaxed', hint: 'Trusted categories run after one approval per plan. YELLOW steps are still held.' },
};

export const TEXT_SIZES = {
  s: { label: 'S', zoom: 0.92 },
  m: { label: 'M', zoom: 1 },
  l: { label: 'L', zoom: 1.12 },
};

export const MODE_PREFERENCES = ['auto', 'always-chill', 'always-hacker'];
export const BACKGROUND_LEVELS = ['off', 'subtle', 'lively'];

export const DEFAULT_UI_SETTINGS = {
  modePreference: 'auto',
  safetyLevel: 'strict',
  textScale: 'm',
  reduceMotion: false,
  speechBubbles: true,
  backgroundIntensity: 'subtle',
  soundEnabled: true,
  masterVolume: 0.35,
};

function sanitizeUiSettings(raw) {
  const s = { ...DEFAULT_UI_SETTINGS, ...raw };
  if (!MODE_PREFERENCES.includes(s.modePreference)) s.modePreference = DEFAULT_UI_SETTINGS.modePreference;
  if (!SAFETY_LEVELS[s.safetyLevel]) s.safetyLevel = DEFAULT_UI_SETTINGS.safetyLevel;
  if (!TEXT_SIZES[s.textScale]) s.textScale = DEFAULT_UI_SETTINGS.textScale;
  if (!BACKGROUND_LEVELS.includes(s.backgroundIntensity)) s.backgroundIntensity = DEFAULT_UI_SETTINGS.backgroundIntensity;
  const volume = Number(s.masterVolume);
  s.masterVolume = Number.isFinite(volume) ? Math.min(1, Math.max(0, volume)) : DEFAULT_UI_SETTINGS.masterVolume;
  s.reduceMotion = Boolean(s.reduceMotion);
  s.speechBubbles = Boolean(s.speechBubbles);
  s.soundEnabled = Boolean(s.soundEnabled);
  return s;
}

export function getUiSettings() {
  return sanitizeUiSettings(readJson(UI_KEY));
}

export function setUiSettings(patch) {
  const next = sanitizeUiSettings({ ...getUiSettings(), ...patch });
  localStorage.setItem(UI_KEY, JSON.stringify(next));
  return next;
}

/** 'strict' | 'balanced' | 'relaxed', as chosen in Settings. */
export function getSafetyLevel() {
  return getUiSettings().safetyLevel;
}

/** Fields to merge into the /api/execute request body. */
export function executePrefsPayload() {
  return { safety_level: getSafetyLevel() };
}

/** True when the user or the OS asked for less motion. */
export function motionReduced() {
  if (getUiSettings().reduceMotion) return true;
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/* ------------------------------------------------------------------ */
/* Last context snapshot (context window viewer)                       */
/* ------------------------------------------------------------------ */

let lastContext = null;
export const getLastContext = () => lastContext;
export const setLastContext = (ctx) => { lastContext = ctx || null; };
