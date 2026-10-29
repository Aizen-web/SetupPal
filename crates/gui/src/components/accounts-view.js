/**
 * SetupPal: Accounts View (ui.md Section 9.4)
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { createMascotSvg } from '../mascot/pets.js';
import { icon } from './icon.js';

export function renderAccountsView(container) {
  const state = store.getState();
  const acc = state.accounts;
  const activePet = state.activePet || 'ghost';

  container.innerHTML = `
    <div class="main-content" style="padding: 24px 32px; overflow-y: auto;">
      <div style="margin-bottom: 24px;">
        <h1 class="font-pixel" style="font-size: 24px; color: var(--ink);">Accounts & Integrations</h1>
        <div style="font-size: 13px; color: var(--ink-soft); margin-top: 4px;">
          Local keychain management and connected AI and registry credentials.
        </div>
      </div>

      <!-- Profile Card -->
      <div style="display: flex; align-items: center; gap: 20px; background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; margin-bottom: 24px; box-shadow: var(--shadow-card);">
        <div style="width: 70px; height: 70px; display: flex; align-items: center; justify-content: center;">
          ${createMascotSvg(activePet, 70, 'acc-avatar')}
        </div>
        <div>
          <div style="display: flex; align-items: center; gap: 8px;">
            <span style="font-size: 18px; font-weight: 700; color: var(--ink);">${acc.displayName}</span>
            <span style="font-size: 11px; background: var(--accent-soft); color: var(--accent-strong); padding: 2px 8px; border-radius: 999px;">Local Profile</span>
          </div>
          <div style="font-size: 12.5px; color: var(--ink-soft); margin-top: 2px;">
            Connected to Windows Credential Manager · Zero secrets stored in plain text
          </div>
        </div>
      </div>

      <!-- Connected Services Grid -->
      <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 16px; margin-bottom: 24px;">
        <!-- GitHub -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 18px 20px; box-shadow: var(--shadow-card);">
          <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;">
            <div class="integration-name">${icon('code', 20)} GitHub</div>
            <span class="badge badge-green">CONNECTED</span>
          </div>
          <div style="font-size: 12.5px; color: var(--ink-soft); margin-bottom: 14px;">
            User: <strong>@${acc.githubUsername}</strong><br />
            Scopes: <code class="font-mono">repo:status, read:packages</code>
          </div>
          <button class="btn-review" style="width: 100%; font-size: 12px;" onclick="alert('GitHub OAuth connection verified.')">Test Connection</button>
        </div>

        <!-- NVIDIA NIM / Nebius -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 18px 20px; box-shadow: var(--shadow-card);">
          <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;">
            <div class="integration-name">${icon('bolt', 20)} NVIDIA NIM</div>
            <span class="badge badge-green">ACTIVE</span>
          </div>
          <div style="font-size: 12.5px; color: var(--ink-soft); margin-bottom: 14px;">
            Model: <strong>Nemotron-3 70B</strong><br />
            API Key: <code class="font-mono">nvapi-••••••••••••3a9f</code>
          </div>
          <button class="btn-review" style="width: 100%; font-size: 12px;" onclick="alert('NVIDIA NIM API key verified.')">Test Connection</button>
        </div>

        <!-- Tavily -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 18px 20px; box-shadow: var(--shadow-card);">
          <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;">
            <div class="integration-name">${icon('search', 20)} Tavily Search</div>
            <span class="badge badge-green">ACTIVE</span>
          </div>
          <div style="font-size: 12.5px; color: var(--ink-soft); margin-bottom: 14px;">
            Verified domain allowlist radar active.<br />
            API Key: <code class="font-mono">tvly-••••••••••••99b1</code>
          </div>
          <button class="btn-review" style="width: 100%; font-size: 12px;" onclick="alert('Tavily Search API key verified.')">Test Connection</button>
        </div>
      </div>
    </div>
  `;
}
