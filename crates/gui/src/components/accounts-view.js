/**
 * SetupPal: Accounts View
 * Just a simple place to paste API keys. Keys stay on this device (local storage).
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { apiClient } from '../api/client.js';
import { icon } from './icon.js';

const KEYS = [
  {
    id: 'nebius',
    storage: null,
    name: 'Nebius Token Factory',
    badge: 'Provider option',
    hint: 'Use NEBIUS_API_KEY in your local .env file. The key is never saved in browser storage.',
    placeholder: 'Paste a key to test it once',
    testable: true,
    environmentOnly: true,
  },
  {
    id: 'nim',
    storage: 'setuppal_ai_api_key',
    name: 'NVIDIA API key',
    badge: 'Provider option',
    hint: 'Powers the chat. Use NIM_API_KEY or NVIDIA_API_KEY in .env, or save the key here.',
    placeholder: 'nvapi-...',
    testable: true,
  },
  {
    id: 'tavily',
    storage: 'setuppal_tavily_key',
    name: 'Tavily API key',
    badge: 'Optional',
    hint: 'Live web checks for extra safety. Works without it.',
    placeholder: 'tvly-...',
  },
  {
    id: 'github',
    storage: 'setuppal_github_user',
    name: 'GitHub token',
    badge: 'Optional',
    hint: 'Higher GitHub rate limits when fetching releases.',
    placeholder: 'ghp_...',
  },
];

const esc = (s) => String(s ?? '').replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const mask = (k) => (k.length > 10 ? `${k.slice(0, 5)}••••••••${k.slice(-4)}` : '••••••••');

function syncAccountsState(id, value) {
  if (id === 'tavily') store.setState(s => ({ ...s, accounts: { ...s.accounts, tavilyConnected: Boolean(value) } }));
  if (id === 'github') {
    store.setState(s => ({ ...s, accounts: { ...s.accounts, githubConnected: Boolean(value), githubUsername: value || '' } }));
  }
}

export function renderAccountsView(container) {
  const health = apiClient.lastHealth;

  const rows = KEYS.map(k => {
    const saved = k.storage ? (localStorage.getItem(k.storage) || '').trim() : '';
    const envConfigured = k.id === 'nim' ? health?.has_env_key
      : k.id === 'nebius' ? health?.has_nebius_key
        : k.id === 'tavily' ? health?.has_tavily_key : false;
    const status = saved
      ? `<span style="color: #16a34a; font-weight: 600;">● Saved (${esc(mask(saved))})</span>`
      : envConfigured
        ? `<span style="color: #16a34a; font-weight: 600;">● Using environment key</span>`
        : `<span style="color: var(--ink-soft);">○ Not set</span>`;
    return `
      <div class="acc-key-card" data-key="${k.id}" style="background: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 18px 22px; box-shadow: var(--shadow-card);">
        <div style="display: flex; align-items: center; justify-content: space-between; gap: 12px;">
          <div style="font-weight: 700; font-size: 15px; color: var(--ink);">${k.name}
            <span style="font-size: 10.5px; font-weight: 600; margin-left: 8px; padding: 2px 8px; border-radius: 999px; background: var(--accent-soft); color: var(--accent-strong);">${k.badge}</span>
          </div>
          <div style="font-size: 12px;">${status}</div>
        </div>
        <div style="font-size: 12px; color: var(--ink-soft); margin: 4px 0 12px;">${k.hint}</div>
        <div style="display: flex; gap: 8px;">
          <input data-input type="password" autocomplete="off" placeholder="${k.placeholder}" style="flex: 1; background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 8px 12px; border-radius: var(--radius-sm); font-size: 13px; font-family: var(--font-mono); outline: none;" />
          <button data-vis class="btn-review" type="button" style="padding: 6px 10px;" title="Show" aria-label="Show">${icon('eye', 18)}</button>
          ${k.environmentOnly ? '' : '<button data-save class="btn-approve" type="button" style="padding: 6px 16px; font-size: 12px;">Save</button>'}
        </div>
        <div style="display: flex; align-items: center; justify-content: space-between; margin-top: 10px; min-height: 26px;">
          <div data-msg style="font-size: 12px; word-break: break-word; max-width: 70%;"></div>
          <div style="display: flex; gap: 8px;">
            ${k.testable ? '<button data-test class="btn-review" type="button" style="padding: 5px 12px; font-size: 12px;">Test</button>' : ''}
            ${k.id === 'nebius' ? '<button data-models class="btn-review" type="button" style="padding: 5px 12px; font-size: 12px;">List models</button>' : ''}
            ${saved ? '<button data-remove class="btn-review" type="button" style="padding: 5px 12px; font-size: 12px; color: var(--v-red);">Remove</button>' : ''}
          </div>
        </div>
      </div>`;
  }).join('');

  container.innerHTML = `
    <div class="main-content" style="padding: 24px 32px; overflow-y: auto;">
      <div style="margin-bottom: 20px;">
        <h1 class="font-pixel" style="font-size: 24px; color: var(--ink);">Accounts</h1>
        <div style="font-size: 13px; color: var(--ink-soft); margin-top: 4px;">
          Chat needs either a Nebius or NVIDIA key, not both. Nebius can use a key from .env; keys entered here stay in this browser's local storage.
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
    if (saveBtn) saveBtn.onclick = () => {
      const val = input.value.trim();
      if (!val) return setMsg('Paste a key first.', 'var(--v-red)');
      localStorage.setItem(k.storage, val);
      syncAccountsState(k.id, val);
      sounds.play('task.done');
      renderAccountsView(container);
    };

    input.onkeydown = (e) => {
      if (e.key !== 'Enter') return;
      if (saveBtn) saveBtn.click();
      else card.querySelector('[data-test]')?.click();
    };

    const removeBtn = card.querySelector('[data-remove]');
    if (removeBtn) {
      removeBtn.onclick = () => {
        if (k.storage) localStorage.removeItem(k.storage);
        syncAccountsState(k.id, '');
        sounds.play('ui.click');
        renderAccountsView(container);
      };
    }

    const testBtn = card.querySelector('[data-test]');
    if (testBtn) {
      testBtn.onclick = async () => {
        const val = input.value.trim() || (k.storage ? localStorage.getItem(k.storage) : '') || '';
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

    const modelsBtn = card.querySelector('[data-models]');
    if (modelsBtn) {
      modelsBtn.onclick = async () => {
        modelsBtn.disabled = true;
        setMsg('Loading the live Nebius model list...', 'var(--accent)');
        try {
          const result = await apiClient.getModels('nebius');
          const models = Array.isArray(result.models) ? result.models : [];
          setMsg(models.length ? models.join(' · ') : 'No models were returned.', models.length ? 'var(--ink-soft)' : 'var(--v-red)');
        } catch (error) {
          setMsg(error.message, 'var(--v-red)');
        } finally {
          modelsBtn.disabled = false;
        }
      };
    }
  });
}
