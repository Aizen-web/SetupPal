/**
 * SetupPal: chat preferences + last-request context snapshot.
 * Kept tiny and separate from the main store so the chat API can read it anywhere.
 */

const KEY = 'setuppal_chat_prefs';

export const REPLY_LENGTHS = {
  short: { label: 'Short', tokens: 400 },
  normal: { label: 'Normal', tokens: 800 },
  long: { label: 'Long', tokens: 1400 },
};

export const DEFAULT_PREFS = {
  memoryTurns: 6, // recent messages sent word for word; older ones become compact notes
  replyLength: 'normal',
  includePcFacts: true,
};

export function getPrefs() {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) || '{}');
    return { ...DEFAULT_PREFS, ...raw };
  } catch {
    return { ...DEFAULT_PREFS };
  }
}

export function setPrefs(patch) {
  const next = { ...getPrefs(), ...patch };
  localStorage.setItem(KEY, JSON.stringify(next));
  return next;
}

export function resetPrefs() {
  localStorage.removeItem(KEY);
  return { ...DEFAULT_PREFS };
}

/** Fields the backend expects on /api/chat. */
export function chatPrefsPayload() {
  const p = getPrefs();
  return {
    memory_turns: Number(p.memoryTurns) || DEFAULT_PREFS.memoryTurns,
    max_reply_tokens: (REPLY_LENGTHS[p.replyLength] || REPLY_LENGTHS.normal).tokens,
    include_pc_facts: Boolean(p.includePcFacts),
  };
}

let lastContext = null;
export const getLastContext = () => lastContext;
export const setLastContext = (ctx) => { lastContext = ctx || null; };
