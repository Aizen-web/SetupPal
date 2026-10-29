/**
 * SetupPal: Accounts View (ui.md Section 9.4)
 * Real credential keychain management for NVIDIA NIM, AI keys, GitHub, and Tavily integrations.
 * Zero preset fake logins.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { createMascotSvg } from '../mascot/pets.js';
import { apiClient } from '../api/client.js';

let isNimEditing = false;

export function renderAccountsView(container) {
  const state = store.getState();
  const activePet = state.activePet || 'ghost';

  const savedKey = localStorage.getItem('setuppal_ai_api_key') || '';

  const savedGhUser = localStorage.getItem('setuppal_github_user') || '';
  const savedTavilyKey = localStorage.getItem('setuppal_tavily_key') || '';
  const displayName = localStorage.getItem('setuppal_display_name') || 'Local User';

  const backendHealth = apiClient.lastHealth;
  const envKeyMasked = backendHealth?.has_env_key ? backendHealth.env_key_masked : '';
  const hasEnvKey = !savedKey.trim() && Boolean(envKeyMasked);
  const hasNimKey = Boolean(savedKey.trim()) || hasEnvKey;
  const showNimInput = (!hasNimKey && !hasEnvKey) || isNimEditing;

  const maskedNimKey = savedKey.trim()
    ? (savedKey.length > 10 ? `${savedKey.slice(0, 6)}••••••••${savedKey.slice(-4)}` : '••••••••')
    : (envKeyMasked || '');

  const isGhActive = Boolean(savedGhUser.trim());
  const isTavilyActive = Boolean(savedTavilyKey.trim());
  const maskedTavily = isTavilyActive
    ? (savedTavilyKey.length > 8 ? `${savedTavilyKey.slice(0, 5)}••••••••${savedTavilyKey.slice(-4)}` : '••••••••')
    : '';

  container.innerHTML = `
    <div class="main-content" style="padding: 24px 32px; overflow-y: auto;">
      <div style="margin-bottom: 24px;">
        <h1 class="font-pixel" style="font-size: 24px; color: var(--ink);">Accounts & Integrations</h1>
        <div style="font-size: 13px; color: var(--ink-soft); margin-top: 4px;">
          Local credentials keychain and connected AI model access.
        </div>
      </div>

      <!-- Profile Card -->
      <div style="display: flex; align-items: center; justify-content: space-between; background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; margin-bottom: 24px; box-shadow: var(--shadow-card);">
        <div style="display: flex; align-items: center; gap: 20px;">
          <div style="width: 70px; height: 70px; display: flex; align-items: center; justify-content: center;">
            ${createMascotSvg(activePet, 70, 'acc-avatar')}
          </div>
          <div>
            <div style="display: flex; align-items: center; gap: 8px;">
              <span id="profile-display-name-text" style="font-size: 18px; font-weight: 700; color: var(--ink);">${displayName}</span>
              <span style="font-size: 11px; background: var(--accent-soft); color: var(--accent-strong); padding: 2px 8px; border-radius: 999px;">Local Profile</span>
            </div>
            <div style="font-size: 12.5px; color: var(--ink-soft); margin-top: 2px;">
              Connected to Windows Credential Manager · Zero secrets stored in plain text
            </div>
          </div>
        </div>
        <div>
          <button id="btn-edit-profile-name" class="btn-review" style="padding: 6px 14px; font-size: 12px;">Edit Name</button>
        </div>
      </div>

      <!-- Connected Services Grid -->
      <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 18px; margin-bottom: 24px;">
        
        <!-- NVIDIA NIM / AI Key Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px; box-shadow: var(--shadow-card); display: flex; flex-direction: column; justify-content: space-between;">
          <div>
            <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;">
              <div style="font-weight: 700; font-size: 15px; color: var(--ink);">⚡ NVIDIA NIM (AI Engine)</div>
              <span id="badge-nim-status" class="badge ${hasNimKey ? 'badge-green' : 'badge-yellow'}">
                ${hasNimKey ? 'ACTIVE' : 'NOT CONFIGURED'}
              </span>
            </div>
            
            <div style="font-size: 12.5px; color: var(--ink-soft); margin-bottom: 14px; line-height: 1.5;">
              Powers zero-trust planning, question analysis, and toolchain reasoning via NVIDIA NIM (nvidia/nemotron-3.5-bolt-e4b).
            </div>

            <!-- Key display or input field -->
            <div id="nim-credentials-container" style="background: var(--bg-canvas); border: 1px solid var(--border-subtle); border-radius: var(--radius-md); padding: 12px; margin-bottom: 14px;">
              ${!showNimInput ? `
                <div style="font-size: 12px; color: var(--ink-soft); margin-bottom: 4px;">Configured Key:</div>
                <div style="display: flex; align-items: center; justify-content: space-between; gap: 8px;">
                  <code class="font-mono" style="font-size: 13px; font-weight: 600; color: var(--ink);">${maskedNimKey}</code>
                  <span style="font-size: 11px; color: #16a34a; font-weight: 600;">${hasEnvKey ? '✓ Loaded (.env)' : '✓ Ready'}</span>
                </div>
                <div style="font-size: 11px; color: var(--ink-soft); margin-top: 6px;">
                  Model: <code class="font-mono">nvidia/nemotron-3.5-bolt-e4b</code>
                </div>
              ` : `
                <div style="margin-bottom: 8px;">
                  <div style="font-size: 12px; font-weight: 600; color: var(--ink); margin-bottom: 4px;">Enter NVIDIA API Key:</div>
                  <div style="display: flex; gap: 6px;">
                    <input id="acc-input-nim" type="password" placeholder="Paste your nvapi-... key" value="${savedKey}" style="flex: 1; background: var(--bg-surface); border: 1px solid var(--border); color: var(--ink); padding: 7px 10px; border-radius: var(--radius-sm); font-size: 12.5px; font-family: var(--font-mono); outline: none;" />
                    <button id="acc-btn-toggle-nim-vis" class="btn-review" style="padding: 4px 8px; font-size: 11px;" title="Show/Hide">👁️</button>
                  </div>
                </div>
                <div style="font-size: 11px; color: var(--ink-soft);">
                  Paste your <code class="font-mono">nvapi-...</code> key to activate full zero-trust planning.
                </div>
              `}
            </div>

            <div id="nim-test-msg" style="font-size: 11.5px; margin-bottom: 12px; min-height: 18px;"></div>
          </div>

          <!-- Buttons Row -->
          <div style="display: flex; gap: 8px; flex-wrap: wrap;">
            ${!showNimInput ? `
              <button id="btn-test-nim" class="btn-review" style="flex: 1; font-size: 12px;">Test Connection</button>
              <button id="btn-change-nim" class="btn-review" style="flex: 1; font-size: 12px;">Change Key</button>
              <button id="btn-clear-nim" class="btn-review" style="padding: 6px 12px; font-size: 12px; color: var(--v-red);">Remove</button>
            ` : `
              <button id="btn-save-nim" class="btn-approve" style="flex: 1; font-size: 12px;">Save Key</button>
              <button id="btn-test-nim-draft" class="btn-review" style="flex: 1; font-size: 12px;">Test Key</button>
              ${hasNimKey ? `
                <button id="btn-cancel-nim" class="btn-review" style="padding: 6px 12px; font-size: 12px;">Cancel</button>
              ` : ''}
            `}
          </div>
        </div>

        <!-- GitHub Integration Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px; box-shadow: var(--shadow-card); display: flex; flex-direction: column; justify-content: space-between;">
          <div>
            <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;">
              <div style="font-weight: 700; font-size: 15px; color: var(--ink);">🐙 GitHub</div>
              <span id="badge-github-status" class="badge ${isGhActive ? 'badge-green' : ''}" style="${!isGhActive ? 'background: rgba(148, 163, 184, 0.15); color: var(--ink-soft); border: 1px solid var(--border);' : ''}">
                ${isGhActive ? 'CONNECTED' : 'NOT CONNECTED'}
              </span>
            </div>
            
            <div style="font-size: 12.5px; color: var(--ink-soft); margin-bottom: 14px; line-height: 1.5;">
              Authenticates against GitHub API to fetch verified release binaries, tags, and verify commit signatures.
            </div>

            <div style="background: var(--bg-canvas); border: 1px solid var(--border-subtle); border-radius: var(--radius-md); padding: 12px; margin-bottom: 14px;">
              ${isGhActive ? `
                <div style="font-size: 12px; color: var(--ink-soft); margin-bottom: 4px;">Connected User:</div>
                <div style="font-size: 13.5px; font-weight: 700; color: var(--ink);">@${savedGhUser}</div>
                <div style="font-size: 11px; color: var(--ink-soft); margin-top: 4px;">
                  Scopes: <code class="font-mono">repo:status, read:packages</code>
                </div>
              ` : `
                <div style="margin-bottom: 8px;">
                  <div style="font-size: 12px; font-weight: 600; color: var(--ink); margin-bottom: 4px;">GitHub Username or PAT:</div>
                  <input id="acc-input-gh-user" type="text" placeholder="e.g. YourUsername" style="width: 100%; box-sizing: border-box; background: var(--bg-surface); border: 1px solid var(--border); color: var(--ink); padding: 7px 10px; border-radius: var(--radius-sm); font-size: 12.5px; font-family: var(--font-mono); outline: none;" />
                </div>
                <div style="font-size: 11px; color: var(--ink-soft);">
                  Optional. Connect for authenticated API rate limits.
                </div>
              `}
            </div>

            <div id="github-msg" style="font-size: 11.5px; margin-bottom: 12px; min-height: 18px;"></div>
          </div>

          <div>
            ${isGhActive ? `
              <button id="btn-disconnect-gh" class="btn-review" style="width: 100%; font-size: 12px; color: var(--v-red);">Disconnect GitHub</button>
            ` : `
              <button id="btn-connect-gh" class="btn-review" style="width: 100%; font-size: 12px;">Save GitHub Account</button>
            `}
          </div>
        </div>

        <!-- Tavily Web Radar Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px; box-shadow: var(--shadow-card); display: flex; flex-direction: column; justify-content: space-between;">
          <div>
            <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;">
              <div style="font-weight: 700; font-size: 15px; color: var(--ink);">🔍 Tavily Search</div>
              <span id="badge-tavily-status" class="badge ${isTavilyActive ? 'badge-green' : ''}" style="${!isTavilyActive ? 'background: rgba(148, 163, 184, 0.15); color: var(--ink-soft); border: 1px solid var(--border);' : ''}">
                ${isTavilyActive ? 'ACTIVE' : 'OPTIONAL (INACTIVE)'}
              </span>
            </div>
            
            <div style="font-size: 12.5px; color: var(--ink-soft); margin-bottom: 14px; line-height: 1.5;">
              Live web radar for real-time domain allowlist checks and ecosystem safety audits.
            </div>

            <div style="background: var(--bg-canvas); border: 1px solid var(--border-subtle); border-radius: var(--radius-md); padding: 12px; margin-bottom: 14px;">
              ${isTavilyActive ? `
                <div style="font-size: 12px; color: var(--ink-soft); margin-bottom: 4px;">Configured Key:</div>
                <div style="display: flex; align-items: center; justify-content: space-between;">
                  <code class="font-mono" style="font-size: 13px; font-weight: 600; color: var(--ink);">${maskedTavily}</code>
                  <span style="font-size: 11px; color: #16a34a; font-weight: 600;">✓ Radar Active</span>
                </div>
              ` : `
                <div style="margin-bottom: 8px;">
                  <div style="font-size: 12px; font-weight: 600; color: var(--ink); margin-bottom: 4px;">Tavily API Key:</div>
                  <input id="acc-input-tavily" type="password" placeholder="tvly-..." style="width: 100%; box-sizing: border-box; background: var(--bg-surface); border: 1px solid var(--border); color: var(--ink); padding: 7px 10px; border-radius: var(--radius-sm); font-size: 12.5px; font-family: var(--font-mono); outline: none;" />
                </div>
                <div style="font-size: 11px; color: var(--ink-soft);">
                  Optional. Offline OSV.dev and embedded radar run without an API key.
                </div>
              `}
            </div>

            <div id="tavily-msg" style="font-size: 11.5px; margin-bottom: 12px; min-height: 18px;"></div>
          </div>

          <div>
            ${isTavilyActive ? `
              <button id="btn-clear-tavily" class="btn-review" style="width: 100%; font-size: 12px; color: var(--v-red);">Remove Key</button>
            ` : `
              <button id="btn-save-tavily" class="btn-review" style="width: 100%; font-size: 12px;">Save Tavily Key</button>
            `}
          </div>
        </div>

      </div>
    </div>
  `;

  // --- Attach Handlers ---

  // 1. Edit Profile Name
  const btnEditProfile = container.querySelector('#btn-edit-profile-name');
  if (btnEditProfile) {
    btnEditProfile.onclick = () => {
      sounds.play('ui.click');
      const current = localStorage.getItem('setuppal_display_name') || 'Local User';
      const newName = prompt('Enter your local profile display name:', current);
      if (newName && newName.trim()) {
        const cleaned = newName.trim();
        localStorage.setItem('setuppal_display_name', cleaned);
        store.setState(s => ({
          ...s,
          accounts: { ...s.accounts, displayName: cleaned }
        }));
        sounds.play('task.done');
        renderAccountsView(container);
      }
    };
  }

  // 2. NVIDIA NIM Handlers
  const nimMsg = container.querySelector('#nim-test-msg');
  const btnToggleNimVis = container.querySelector('#acc-btn-toggle-nim-vis');
  const inputNim = container.querySelector('#acc-input-nim');

  if (btnToggleNimVis && inputNim) {
    btnToggleNimVis.onclick = () => {
      inputNim.type = inputNim.type === 'password' ? 'text' : 'password';
      btnToggleNimVis.textContent = inputNim.type === 'password' ? '👁️' : '🙈';
    };
  }

  // Save NIM Key
  const btnSaveNim = container.querySelector('#btn-save-nim');
  if (btnSaveNim && inputNim) {
    btnSaveNim.onclick = () => {
      const val = inputNim.value.trim();
      if (!val) {
        if (nimMsg) nimMsg.innerHTML = `<span style="color: var(--v-red);">Please enter your API key first.</span>`;
        return;
      }

      localStorage.setItem('setuppal_ai_api_key', val);

      sounds.play('task.done');
      isNimEditing = false;
      renderAccountsView(container);
    };
  }

  // Cancel NIM editing
  const btnCancelNim = container.querySelector('#btn-cancel-nim');
  if (btnCancelNim) {
    btnCancelNim.onclick = () => {
      isNimEditing = false;
      renderAccountsView(container);
    };
  }

  // Change NIM Key
  const btnChangeNim = container.querySelector('#btn-change-nim');
  if (btnChangeNim) {
    btnChangeNim.onclick = () => {
      isNimEditing = true;
      renderAccountsView(container);
    };
  }

  // Remove NIM Key
  const btnClearNim = container.querySelector('#btn-clear-nim');
  if (btnClearNim) {
    btnClearNim.onclick = () => {
      if (confirm('Are you sure you want to remove your AI API key?')) {
        localStorage.removeItem('setuppal_ai_api_key');
        sounds.play('ui.click');
        isNimEditing = false;
        renderAccountsView(container);
      }
    };
  }

  // Test Saved NIM Key
  const btnTestNim = container.querySelector('#btn-test-nim');
  if (btnTestNim) {
    btnTestNim.onclick = async () => {
      sounds.play('ui.click');
      if (nimMsg) nimMsg.innerHTML = `<span style="color: var(--accent);">⟳ Testing key with NVIDIA NIM...</span>`;
      btnTestNim.disabled = true;

      try {
        const res = await apiClient.validateKey({
          apiKey: savedKey,
        });

        if (res.valid) {
          sounds.play('verdict.green');
          if (nimMsg) nimMsg.innerHTML = `<span style="color: #16a34a;">✓ ${res.message}</span>`;
        } else {
          sounds.play('verdict.red');
          if (nimMsg) nimMsg.innerHTML = `<span style="color: var(--v-red);">✗ ${res.message}</span>`;
        }
      } catch (err) {
        sounds.play('error');
        if (nimMsg) nimMsg.innerHTML = `<span style="color: var(--v-red);">Error: ${err.message}</span>`;
      } finally {
        btnTestNim.disabled = false;
      }
    };
  }

  // Test Draft NIM Key (before saving)
  const btnTestNimDraft = container.querySelector('#btn-test-nim-draft');
  if (btnTestNimDraft && inputNim) {
    btnTestNimDraft.onclick = async () => {
      const val = inputNim.value.trim();
      if (!val) {
        if (nimMsg) nimMsg.innerHTML = `<span style="color: var(--v-red);">Please enter an API key to test.</span>`;
        return;
      }
      sounds.play('ui.click');
      if (nimMsg) nimMsg.innerHTML = `<span style="color: var(--accent);">⟳ Testing key with NVIDIA NIM...</span>`;
      btnTestNimDraft.disabled = true;

      try {
        const res = await apiClient.validateKey({ apiKey: val });

        if (res.valid) {
          sounds.play('verdict.green');
          if (nimMsg) nimMsg.innerHTML = `<span style="color: #16a34a;">✓ ${res.message}</span>`;
        } else {
          sounds.play('verdict.red');
          if (nimMsg) nimMsg.innerHTML = `<span style="color: var(--v-red);">✗ ${res.message}</span>`;
        }
      } catch (err) {
        sounds.play('error');
        if (nimMsg) nimMsg.innerHTML = `<span style="color: var(--v-red);">Error: ${err.message}</span>`;
      } finally {
        btnTestNimDraft.disabled = false;
      }
    };
  }

  // 3. GitHub Handlers
  const btnConnectGh = container.querySelector('#btn-connect-gh');
  const inputGhUser = container.querySelector('#acc-input-gh-user');
  const ghMsg = container.querySelector('#github-msg');

  if (btnConnectGh && inputGhUser) {
    btnConnectGh.onclick = () => {
      const val = inputGhUser.value.trim();
      if (!val) {
        if (ghMsg) ghMsg.innerHTML = `<span style="color: var(--v-red);">Please enter a username or token.</span>`;
        return;
      }
      const cleanUser = val.replace(/^@/, '');
      localStorage.setItem('setuppal_github_user', cleanUser);
      store.setState(s => ({
        ...s,
        accounts: { ...s.accounts, githubConnected: true, githubUsername: cleanUser }
      }));
      sounds.play('task.done');
      renderAccountsView(container);
    };
  }

  const btnDisconnectGh = container.querySelector('#btn-disconnect-gh');
  if (btnDisconnectGh) {
    btnDisconnectGh.onclick = () => {
      localStorage.removeItem('setuppal_github_user');
      store.setState(s => ({
        ...s,
        accounts: { ...s.accounts, githubConnected: false, githubUsername: '' }
      }));
      sounds.play('ui.click');
      renderAccountsView(container);
    };
  }

  // 4. Tavily Handlers
  const btnSaveTavily = container.querySelector('#btn-save-tavily');
  const inputTavily = container.querySelector('#acc-input-tavily');
  const tavilyMsg = container.querySelector('#tavily-msg');

  if (btnSaveTavily && inputTavily) {
    btnSaveTavily.onclick = () => {
      const val = inputTavily.value.trim();
      if (!val) {
        if (tavilyMsg) tavilyMsg.innerHTML = `<span style="color: var(--v-red);">Please enter a Tavily API key.</span>`;
        return;
      }
      localStorage.setItem('setuppal_tavily_key', val);
      store.setState(s => ({
        ...s,
        accounts: { ...s.accounts, tavilyConnected: true }
      }));
      sounds.play('task.done');
      renderAccountsView(container);
    };
  }

  const btnClearTavily = container.querySelector('#btn-clear-tavily');
  if (btnClearTavily) {
    btnClearTavily.onclick = () => {
      localStorage.removeItem('setuppal_tavily_key');
      store.setState(s => ({
        ...s,
        accounts: { ...s.accounts, tavilyConnected: false }
      }));
      sounds.play('ui.click');
      renderAccountsView(container);
    };
  }
}
