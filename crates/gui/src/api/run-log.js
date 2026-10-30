/**
 * Pure helpers for the run log: dry-run labelling and the once-per-conversation scan line.
 * No DOM or network access, so scripts/check-run-log.js can test them directly.
 */

export const DRY_RUN_LABEL = 'DRY RUN';

/** Prefixes a terminal line so a simulated step can never read like a real one. */
export function labelRunText(text, dryRun) {
  return dryRun ? `[${DRY_RUN_LABEL}] ${text}` : text;
}

/** Text for the final line of a run. Uses the backend headline when it sent one. */
export function runSummaryText({ dryRun, passed, headline }) {
  if (headline) return dryRun && !headline.startsWith(DRY_RUN_LABEL) ? labelRunText(headline, true) : headline;
  if (dryRun) {
    return passed
      ? `${DRY_RUN_LABEL} finished: nothing was installed or changed on this PC.`
      : `${DRY_RUN_LABEL} found a problem: nothing was installed or changed on this PC.`;
  }
  return passed ? 'All approved tasks executed and verified cleanly.' : 'Run failed.';
}

/** Order-independent fingerprint of the detected tools (name plus first version line). */
export function scanSignature(profile) {
  const tools = Array.isArray(profile?.installed_tools) ? profile.installed_tools : [];
  return tools
    .map(t => `${typeof t?.binary === 'string' ? t.binary : JSON.stringify(t?.binary ?? '')}:${String(t?.version_output ?? '').split('\n')[0].trim()}`)
    .sort()
    .join('|');
}

/**
 * Returns the "detected N tools" log entry, or null when the newest scan line in this
 * conversation already describes the same facts. Real scan failures never go through here,
 * so they are never hidden.
 */
export function scanLogEntry(logs, profile) {
  const count = Array.isArray(profile?.installed_tools) ? profile.installed_tools.length : 0;
  if (!count) return null;
  const signature = scanSignature(profile);
  const previous = [...(logs || [])].reverse().find(entry => entry?.scanSignature !== undefined);
  if (previous && previous.scanSignature === signature) return null;
  return {
    id: Date.now() + Math.random(),
    text: `~ detected ${count} installed tool${count === 1 ? '' : 's'}`,
    type: 'info',
    scanSignature: signature,
  };
}
