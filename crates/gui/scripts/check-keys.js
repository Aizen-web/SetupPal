// Checks the localStorage -> keychain key migration and that the client never stores keys locally.
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { keychainNameFor, migrateLegacyKeys, LEGACY_KEY_STORAGE } from '../src/state/key-migration.js';

function fakeStorage(initial) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: k => (map.has(k) ? map.get(k) : null),
    removeItem: k => { map.delete(k); },
    setItem: (k, v) => { map.set(k, v); },
    keys: () => [...map.keys()],
  };
}

assert.equal(keychainNameFor('setuppal_ai_api_key', 'nvapi-abc'), 'nim_api_key');
assert.equal(keychainNameFor('setuppal_ai_api_key', 'sk-other'), 'nebius_api_key');
assert.equal(keychainNameFor('setuppal_tavily_key', 'tvly-1'), 'tavily_api_key');
assert.equal(keychainNameFor('setuppal_github_user', 'ghp_abc'), 'github_token');
assert.equal(keychainNameFor('setuppal_github_user', 'octocat'), null);
assert.equal(keychainNameFor('setuppal_ai_api_key', '   '), null);

// Saved values are removed; the backend receives each one once.
const saved = [];
const storage = fakeStorage({
  setuppal_ai_api_key: 'nvapi-test',
  setuppal_tavily_key: 'tvly-test',
  setuppal_github_user: 'octocat',
  setuppal_display_name: 'Local User',
});
const result = await migrateLegacyKeys(async (name, value) => { saved.push([name, value]); }, storage);
assert.deepEqual(saved, [['nim_api_key', 'nvapi-test'], ['tavily_api_key', 'tvly-test']]);
assert.deepEqual(result, { migrated: 2, removed: 1, failed: 0 });
assert.deepEqual(storage.keys(), ['setuppal_display_name']);

// A failed save keeps the value so the next launch can retry.
const retry = fakeStorage({ setuppal_tavily_key: 'tvly-keep' });
const failed = await migrateLegacyKeys(async () => { throw new Error('backend down'); }, retry);
assert.equal(failed.failed, 1);
assert.equal(retry.getItem('setuppal_tavily_key'), 'tvly-keep');
assert.equal(LEGACY_KEY_STORAGE.length, 3);

// No source file may write a key-like value into localStorage.
const srcDir = fileURLToPath(new URL('../src', import.meta.url));
function* files(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* files(path);
    else if (path.endsWith('.js')) yield path;
  }
}
for (const file of files(srcDir)) {
  const text = readFileSync(file, 'utf8');
  assert.ok(
    !/localStorage\.setItem\([^)]*(api_?key|tavily|github|token)/i.test(text),
    `${file} writes a key-like value to localStorage`,
  );
  assert.ok(!/new\s+EventSource\(/.test(text), `${file} uses EventSource, which cannot send the bearer token`);
}

console.log('Key migration checks passed.');
