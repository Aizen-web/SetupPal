/**
 * SetupPal: Accounts View
 * Keys are write-only here. They are saved to the OS keychain (Windows Credential Manager) by the
 * backend and are never shown again, never kept in localStorage.
 */

import { sounds } from '../audio/sound-engine.js';
import { apiClient } from '../api/client.js';
import { currentKeyStatus, refreshKeyStatus } from '../state/key-status.js';
import { icon } from './icon.js';
import { esc } from './html.js';

const KEYS = [
  {
    id: 'nebius',
    keyName: 'nebius_api_key',
    name: 'Nebius Token Factory',
    badge: 'Production',
    hint: 'Used for chat and planning when set. Stored in Windows Credential Manager.',
    placeholder: 'Paste your Nebius key',
    testable: true,
  },
  {
    id: 'nim',
    keyName: 'nim_api_key',
    name: 'NVIDIA NIM',
    badge: 'Testing',
    hint: 'Used when no Nebius key is set, or when SETUPPAL_PROVIDER=nim.',
    placeholder: 'nvapi-...',
    testable: true,
  },
  {
    id: 'tavily',
    keyName: 'tavily_api_key',
    name: 'Tavily API key',
    badge: 'Optional',
    hint: 'Confirms unfamiliar GitHub projects are the official ones, not copies. Without it, those steps need your approval.',
    placeholder: 'tvly-...',
    testable: true,
  },
  {
    id: 'github',
    keyName: 'github_token',
    name: 'GitHub token',
    badge: 'Optional',
    hint: 'Higher GitHub rate limits when fetching releases.',
    placeholder: 'ghp_...',
    testable: true,
  },
];

export function renderAccountsView(container) {
  const health = apiClient.lastHealth;
  const status = currentKeyStatus();

  // Status is fetched once per visit; the view re-renders when it arrives.
  if (!status && !renderAccountsView.loading) {
    renderAccountsView.loading = true;
    refreshKeyStatus()
      .catch(() => {})
      .finally(() => { renderAccountsView.loading = false; });
  }

  const rows = KEYS.map(k => {
    const inKeychain = Boolean(status?.in_keychain?.[k.keyName]);
    const configured = Boolean(status?.configured?.[k.keyName]);
    const isActive = Boolean(health?.provider) && health.provider === k.id;
    const state = !status
      ? `<span style="color: var(--ink-soft);">… Checking</span>`
      : inKeychain
        ? `<span style="color: #16a34a; font-weight: 600;">● Saved</span>`
        : configured
          ? `<span style="color: #16a34a; font-weight: 600;">● Using environment key</span>`
          : `<span style="color: var(--ink-soft);">○ Not set</span>`;
    const activeLine = isActive
      ? `<div style="font-size: 12px; color: var(--ink); margin-top: 4px;">Chat and planning use this provider · Model: <code>${esc(health.model)}</code>${health.provider_ready ? '' : ` · <span style="color: var(--v-red);">${esc(health.provider_error)}</span>`}</div>`
      : '';
    return `
      <div class="acc-key-card" data-key="${k.id}" style="background: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 18px 22px; box-shadow: var(--shadow-card);">
        <div style="display: flex; align-items: center; justify-content: space-between; gap: 12px;">
          <div style="font-weight: 700; font-size: 15px; color: var(--ink);">${esc(k.name)}
            <span style="font-size: 10.5px; font-weight: 600; margin-left: 8px; padding: 2px 8px; border-radius: 999px; background: var(--accent-soft); color: var(--accent-strong);">${esc(k.badge)}</span>
          </div>
          <div style="font-size: 12px;">${state}</div>
        </div>
        ${activeLine}
        <div style="font-size: 12px; color: var(--ink-soft); margin: 4px 0 12px;">${esc(k.hint)}</div>
        <div style="display: flex; gap: 8px;">
          <input data-input type="password" autocomplete="off" spellcheck="false" placeholder="${esc(inKeychain ? 'Saved. Paste a new key to replace it' : k.placeholder)}" style="flex: 1; background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 8px 12px; border-radius: var(--radius-sm); font-size: 13px; font-family: var(--font-mono); outline: none;" />
          <button data-vis class="btn-review" type="button" style="padding: 6px 10px;" title="Show" aria-label="Show">${icon('eye', 18)}</button>
          <button data-save class="btn-approve" type="button" style="padding: 6px 16px; font-size: 12px;">Save</button>
        </div>
        <div style="display: flex; align-items: center; justify-content: space-between; margin-top: 10px; min-height: 26px;">
          <div data-msg style="font-size: 12px; word-break: break-word; max-width: 70%;"></div>
          <div style="display: flex; gap: 8px;">
            ${k.testable ? '<button data-test class="btn-review" type="button" style="padding: 5px 12px; font-size: 12px;">Test</button>' : ''}
            ${inKeychain ? '<button data-remove class="btn-review" type="button" style="padding: 5px 12px; font-size: 12px; color: var(--v-red);">Remove</button>' : ''}
          </div>
        </div>
      </div>`;
  }).join('');

  container.innerHTML = `
    <div class="main-content" style="padding: 24px 32px; overflow-y: auto;">
      <div style="margin-bottom: 20px;">
        <h1 class="font-pixel" style="font-size: 24px; color: var(--ink);">Accounts</h1>
        <div style="font-size: 13px; color: var(--ink-soft); margin-top: 4px;">
          Chat needs a Nebius or NVIDIA NIM key. Keys are saved to Windows Credential Manager and are never shown again.
        </div>
      </div>
      <div style="display: flex; flex-direction: column; gap: 16px; max-width: 680px;">${rows}</div>
    </div>
  `;

  KEYS.forEach(k => {
    const card = container.querySelector(`.acc-key-card[data-key="${k.id}"]`);
    const input = card.querySelector('[data-input]');
    const msg = card.querySelector('[data-msg]');
    const setMsg = (text, color) => { msg.innerHTML = `<span style="color: ${color};">${esc(text)}</span>`; };

    card.querySelector('[data-vis]').onclick = (e) => {
      input.type = input.type === 'password' ? 'text' : 'password';
      e.currentTarget.innerHTML = icon(input.type === 'password' ? 'eye' : 'eyeOff', 18);
    };

    const saveBtn = card.querySelector('[data-save]');
    saveBtn.onclick = async () => {
      const val = input.value.trim();
      if (!val) return setMsg('Paste a key first.', 'var(--v-red)');
      saveBtn.disabled = true;
      try {
        await apiClient.saveKey(k.keyName, val);
        input.value = '';
        await refreshKeyStatus();
        await apiClient.checkHealth();
        sounds.play('task.done');
        renderAccountsView(container);
      } catch (err) {
        sounds.play('error');
        setMsg(`Could not save: ${err.message}`, 'var(--v-red)');
        saveBtn.disabled = false;
      }
    };

    input.onkeydown = (e) => {
      if (e.key === 'Enter') saveBtn.click();
    };

    const removeBtn = card.querySelector('[data-remove]');
    if (removeBtn) {
      removeBtn.onclick = async () => {
        removeBtn.disabled = true;
        try {
          await apiClient.deleteKey(k.keyName);
          await refreshKeyStatus();
          await apiClient.checkHealth();
          sounds.play('ui.click');
          renderAccountsView(container);
        } catch (err) {
          setMsg(`Could not remove: ${err.message}`, 'var(--v-red)');
          removeBtn.disabled = false;
        }
      };
    }

    const testBtn = card.querySelector('[data-test]');
    if (testBtn) {
      testBtn.onclick = async () => {
        const val = input.value.trim();
        sounds.play('ui.click');
        setMsg('Testing...', 'var(--accent)');
        testBtn.disabled = true;
        try {
          const res = await apiClient.validateKey({ apiKey: val || null, provider: k.id });
          sounds.play(res.valid ? 'verdict.green' : 'verdict.red');
          setMsg(res.message, res.valid ? '#16a34a' : 'var(--v-red)');
        } catch (err) {
          sounds.play('error');
          setMsg(`Error: ${err.message}`, 'var(--v-red)');
        } finally {
          testBtn.disabled = false;
        }
      };
    }
  });
}
