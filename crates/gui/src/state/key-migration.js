/**
 * SetupPal: one-time move of old browser-stored keys into the OS keychain.
 * Older builds kept keys in localStorage. Each one is sent to the backend (POST /api/keys) and then
 * deleted from localStorage. Values are never logged.
 */

export const LEGACY_KEY_STORAGE = ['setuppal_ai_api_key', 'setuppal_tavily_key', 'setuppal_github_user'];

const GITHUB_TOKEN = /^(ghp_|gho_|ghu_|ghs_|ghr_|github_pat_)/;

/** Which keychain entry an old value belongs to, or null when it is not a key (for example a plain username). */
export function keychainNameFor(storageKey, value) {
  const v = String(value || '').trim();
  if (!v) return null;
  if (storageKey === 'setuppal_tavily_key') return 'tavily_api_key';
  if (storageKey === 'setuppal_github_user') return GITHUB_TOKEN.test(v) ? 'github_token' : null;
  if (storageKey === 'setuppal_ai_api_key') return v.startsWith('nvapi-') ? 'nim_api_key' : 'nebius_api_key';
  return null;
}

/**
 * Migrates and removes legacy values. A value is only removed from localStorage after the backend
 * confirmed it was saved; if saving fails it stays for the next launch.
 * @returns {Promise<{migrated: number, removed: number, failed: number}>}
 */
export async function migrateLegacyKeys(saveKey, storage = globalThis.localStorage) {
  const result = { migrated: 0, removed: 0, failed: 0 };
  for (const storageKey of LEGACY_KEY_STORAGE) {
    const value = storage.getItem(storageKey);
    if (value === null) continue;
    const name = keychainNameFor(storageKey, value);
    if (!name) {
      storage.removeItem(storageKey);
      result.removed += 1;
      continue;
    }
    try {
      await saveKey(name, value.trim());
      storage.removeItem(storageKey);
      result.migrated += 1;
    } catch {
      result.failed += 1;
    }
  }
  return result;
}
