/**
 * SetupPal: shared HTML helpers.
 * Every value interpolated into an innerHTML template goes through esc().
 */

const ENTITIES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

export function esc(value) {
  if (value === null || value === undefined) return '';
  return String(value).replace(/[&<>"']/g, ch => ENTITIES[ch]);
}

/** Returns the href only when it is an absolute https URL, otherwise null. */
export function safeHttpsUrl(value) {
  try {
    const url = new URL(String(value || ''));
    return url.protocol === 'https:' ? url.href : null;
  } catch {
    return null;
  }
}
