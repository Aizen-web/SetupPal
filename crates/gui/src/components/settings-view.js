/**
 * SetupPal: Settings View (ui.md Section 9.5)
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { apiClient } from '../api/client.js';

export function renderSettingsView(container) {
  const state = store.getState();
  const settings = state.settings;

  container.innerHTML = `
    <div class="main-content" style="padding: 24px 32px; overflow-y: auto;">
      <div style="margin-bottom: 24px;">
        <h1 class="font-pixel" style="font-size: 24px; color: var(--ink);">Settings</h1>
        <div style="font-size: 13px; color: var(--ink-soft); margin-top: 4px;">
          Configure appearance, safety gates, audio engine, and mascot behavior.
        </div>
      </div>

      <div style="display: flex; flex-direction: column; gap: 20px; max-width: 680px;">
        <!-- Appearance Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin-bottom: 14px;">Appearance & Modes</h2>
          
          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0; border-bottom: 1px solid var(--border-subtle);">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Theme Mode</div>
              <div style="font-size: 12px; color: var(--ink-soft);">Auto-switches to Hacker mode during tool and task execution</div>
            </div>
            <select id="select-mode" style="background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 12px; border-radius: var(--radius-sm); font-size: 13px; outline: none;">
              <option value="auto" ${state.modePreference === 'auto' ? 'selected' : ''}>Auto (Recommended)</option>
              <option value="always-chill" ${state.modePreference === 'always-chill' ? 'selected' : ''}>Always Chill</option>
              <option value="always-hacker" ${state.modePreference === 'always-hacker' ? 'selected' : ''}>Always Hacker</option>
            </select>
          </div>

          <div style="display: flex; align-items: center; justify-content: space-between; padding: 12px 0;">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Manual Override</div>
              <div style="font-size: 12px; color: var(--ink-soft);">Switch active mode immediately</div>
            </div>
            <div style="display: flex; gap: 8px;">
              <button id="btn-force-chill" class="btn-review" style="padding: 6px 12px; font-size: 12px;">Chill</button>
              <button id="btn-force-hacker" class="btn-review" style="padding: 6px 12px; font-size: 12px;">Hacker</button>
            </div>
          </div>
        </div>

        <!-- Zero-Trust Safety Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin-bottom: 14px;">Zero-Trust Safety Engine</h2>
          
          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0;">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Safety Level</div>
              <div style="font-size: 12px; color: var(--ink-soft);">Strict: All plans need approval; YELLOW tasks always held; RED blocked</div>
            </div>
            <select id="select-safety" style="background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 12px; border-radius: var(--radius-sm); font-size: 13px; outline: none;">
              <option value="strict" selected>Strict (Default)</option>
              <option value="balanced">Balanced</option>
              <option value="relaxed">Relaxed</option>
            </select>
          </div>
        </div>

        <!-- AI Model & Zero-Trust Intelligence Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;">
            <div>
              <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin: 0 0 4px 0;">AI Model & Zero-Trust Intelligence</h2>
              <div style="font-size: 12px; color: var(--ink-soft);">Powers dynamic mod research, problem scoping, and custom zero-trust plans.</div>
            </div>
            <div id="ai-key-status-badge">
              <!-- Dynamically populated badge -->
            </div>
          </div>

          <div style="display: flex; flex-direction: column; gap: 14px; margin-top: 14px;">
            <!-- Provider Selector -->
            <div style="display: flex; align-items: center; justify-content: space-between; padding: 6px 0; border-bottom: 1px solid var(--border-subtle);">
              <div>
                <div style="font-weight: 600; font-size: 13px; color: var(--ink);">AI Provider</div>
                <div style="font-size: 11.5px; color: var(--ink-soft);">Select your preferred OpenAI-compatible engine</div>
              </div>
              <select id="select-ai-provider" style="background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 12px; border-radius: var(--radius-sm); font-size: 13px; outline: none;">
                <option value="nim">NVIDIA NIM (Llama 3.1 70B / Nemotron)</option>
                <option value="openai">OpenAI (GPT-4o / GPT-4o-mini)</option>
                <option value="custom">Custom Endpoint</option>
              </select>
            </div>

            <!-- API Key Input -->
            <div style="padding: 6px 0; border-bottom: 1px solid var(--border-subtle);">
              <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
                <div style="font-weight: 600; font-size: 13px; color: var(--ink);">API Key</div>
                <span id="ai-key-hint" style="font-size: 11px; color: var(--ink-soft); font-family: var(--font-mono);">sk-...</span>
              </div>
              <div style="display: flex; gap: 8px;">
                <input id="input-ai-key" type="password" placeholder="Paste your API key here..." style="flex: 1; background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 8px 12px; border-radius: var(--radius-sm); font-size: 13px; font-family: var(--font-mono); outline: none;" />
                <button id="btn-toggle-key-vis" class="btn-review" style="padding: 6px 10px; font-size: 12px;" title="Show/Hide Key">👁️</button>
              </div>
            </div>

            <!-- Model Name Input & Presets -->
            <div style="padding: 8px 0; border-bottom: 1px solid var(--border-subtle);">
              <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 6px;">
                <div>
                  <div style="font-weight: 600; font-size: 13px; color: var(--ink);">Model Identifier</div>
                  <div style="font-size: 11.5px; color: var(--ink-soft);">Select a preset below or enter any model identifier</div>
                </div>
                <input id="input-ai-model" type="text" placeholder="meta/llama-3.3-70b-instruct" style="width: 250px; background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 10px; border-radius: var(--radius-sm); font-size: 12px; font-family: var(--font-mono); outline: none;" />
              </div>
              <div id="model-presets-row" style="display: flex; gap: 6px; flex-wrap: wrap; align-items: center; padding-top: 4px;">
                <span style="font-size: 11px; color: var(--ink-soft); font-weight: 500;">Presets:</span>
                <button type="button" class="btn-model-preset" data-model="meta/llama-3.3-70b-instruct" style="font-size: 11px; padding: 3px 8px; border-radius: var(--radius-pill); border: 1px solid var(--border); background: var(--bg-canvas); color: var(--ink); cursor: pointer;">Llama 3.3 70B ⭐</button>
                <button type="button" class="btn-model-preset" data-model="meta/llama-3.1-8b-instruct" style="font-size: 11px; padding: 3px 8px; border-radius: var(--radius-pill); border: 1px solid var(--border); background: var(--bg-canvas); color: var(--ink); cursor: pointer;">Llama 3.1 8B ⚡</button>
                <button type="button" class="btn-model-preset" data-model="nvidia/llama-3.1-nemotron-70b-instruct" style="font-size: 11px; padding: 3px 8px; border-radius: var(--radius-pill); border: 1px solid var(--border); background: var(--bg-canvas); color: var(--ink); cursor: pointer;">Nemotron 70B</button>
                <button type="button" class="btn-model-preset" data-model="mistralai/mistral-7b-instruct-v0.3" style="font-size: 11px; padding: 3px 8px; border-radius: var(--radius-pill); border: 1px solid var(--border); background: var(--bg-canvas); color: var(--ink); cursor: pointer;">Mistral 7B</button>
                <button type="button" class="btn-model-preset" data-model="gpt-4o-mini" style="font-size: 11px; padding: 3px 8px; border-radius: var(--radius-pill); border: 1px solid var(--border); background: var(--bg-canvas); color: var(--ink); cursor: pointer;">gpt-4o-mini</button>
              </div>
            </div>

            <!-- Endpoint URL Input -->
            <div style="display: flex; align-items: center; justify-content: space-between; padding: 6px 0;">
              <div>
                <div style="font-weight: 600; font-size: 13px; color: var(--ink);">API Endpoint URL</div>
                <div style="font-size: 11.5px; color: var(--ink-soft);">Leave default or specify custom proxy</div>
              </div>
              <input id="input-ai-endpoint" type="text" placeholder="https://api.openai.com/v1/chat/completions" style="width: 260px; background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 10px; border-radius: var(--radius-sm); font-size: 12px; font-family: var(--font-mono); outline: none;" />
            </div>

            <!-- Token Optimization Features Callout -->
            <div style="background: rgba(34, 197, 94, 0.06); border: 1px solid rgba(34, 197, 94, 0.2); border-radius: var(--radius-md); padding: 10px 14px; font-size: 11.5px; line-height: 1.5; color: var(--ink);">
              <div style="font-weight: 600; color: #16a34a; margin-bottom: 3px; display: flex; align-items: center; gap: 6px;">
                <span>⚡ Zero-Waste Token Engine Active</span>
              </div>
              <div style="color: var(--ink-soft);">
                • <strong>Prompt Caching Prefix</strong>: Immutable system prompts trigger OpenAI/NIM cache discounts (up to 50%).<br/>
                • <strong>Compact Context & History Distillation</strong>: XML attribute serialization and sliding window prevent quadratic token inflation.<br/>
                • <strong>Session Deduplication Cache</strong>: Repeat queries consume 0 remote API tokens.
              </div>
            </div>

            <!-- Action Buttons and Feedback -->
            <div style="display: flex; align-items: center; justify-content: space-between; padding-top: 10px; border-top: 1px dashed var(--border-subtle);">
              <div id="ai-save-msg" style="font-size: 12px; font-weight: 500;"></div>
              <div style="display: flex; gap: 8px;">
                <button id="btn-test-ai-key" class="btn-review" style="padding: 6px 14px; font-size: 12px;">Test Connection</button>
                <button id="btn-clear-ai-key" class="btn-review" style="padding: 6px 14px; font-size: 12px;">Clear Key</button>
                <button id="btn-save-ai-key" class="btn-approve" style="padding: 6px 16px; font-size: 12px;">Save Configuration</button>
              </div>
            </div>
          </div>
        </div>

        <!-- System Doctor & Toolchain Scan Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 14px;">
            <h2 class="font-pixel" style="font-size: 17px; color: var(--ink);">System Doctor & Toolchain Scan</h2>
            <button id="btn-scan-doctor" class="btn-review" style="padding: 5px 12px; font-size: 12px;">Run Scan</button>
          </div>
          <div id="doctor-profile-output" style="font-size: 12px; color: var(--ink-soft); line-height: 1.6;">
            Click <strong>Run Scan</strong> to inspect host OS, GPU, and detected development tools via <code class="font-mono">SystemScanner</code>.
          </div>
        </div>

        <!-- Mascot Pet Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin-bottom: 14px;">Active Kawaii Pet Companion</h2>
          
          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0;">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Companion Pet</div>
              <div style="font-size: 12px; color: var(--ink-soft);">Each pet features 20 unique animations and distinct personalities</div>
            </div>
            <select id="select-active-pet" style="background: var(--bg-canvas); border: 1px solid var(--border); color: var(--ink); padding: 6px 12px; border-radius: var(--radius-sm); font-size: 13px; outline: none;">
              <option value="ghost" ${state.activePet === 'ghost' ? 'selected' : ''}>👻 Pixel the Ghost</option>
              <option value="cat" ${state.activePet === 'cat' ? 'selected' : ''}>🐱 Byte the Cat</option>
              <option value="bunny" ${state.activePet === 'bunny' ? 'selected' : ''}>🐰 Mochi the Bunny</option>
            </select>
          </div>
        </div>

        <!-- Sound Engine Card -->
        <div style="background-color: var(--bg-surface); border: 1px solid var(--border); border-radius: var(--radius-lg); padding: 20px 24px; box-shadow: var(--shadow-card);">
          <h2 class="font-pixel" style="font-size: 17px; color: var(--ink); margin-bottom: 14px;">Cozy 8-Bit Audio</h2>
          
          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0;">
            <div>
              <div style="font-weight: 600; font-size: 13.5px; color: var(--ink);">Sound Effects</div>
              <div style="font-size: 12px; color: var(--ink-soft);">Blips, sweeps, dings, and victory fanfare</div>
            </div>
            <input type="checkbox" id="check-sound" ${sounds.enabled ? 'checked' : ''} style="transform: scale(1.3); cursor: pointer;" />
          </div>

          <div style="display: flex; align-items: center; justify-content: space-between; padding: 8px 0; border-top: 1px solid var(--border-subtle); margin-top: 8px;">
            <div style="font-size: 13px; color: var(--ink-soft);">Volume</div>
            <input type="range" id="range-vol" min="0" max="1" step="0.05" value="${sounds.volume}" style="width: 140px; cursor: pointer;" />
          </div>
        </div>
      </div>
    </div>
  `;

  // Attach listeners
  container.querySelector('#select-mode').onchange = (e) => {
    const val = e.target.value;
    store.setState({ modePreference: val });
    if (val === 'always-chill') store.setMode('chill');
    if (val === 'always-hacker') store.setMode('hacker');
  };

  container.querySelector('#btn-force-chill').onclick = () => store.setMode('chill');
  container.querySelector('#btn-force-hacker').onclick = () => store.setMode('hacker');

  container.querySelector('#select-active-pet').onchange = (e) => {
    store.setActivePet(e.target.value);
  };

  container.querySelector('#select-safety').onchange = (e) => {
    store.setState(s => ({ ...s, settings: { ...s.settings, safetyLevel: e.target.value } }));
    sounds.play('ui.click');
  };

  container.querySelector('#check-sound').onchange = (e) => {
    sounds.setMuted(!e.target.checked);
    if (e.target.checked) sounds.play('task.done');
  };

  container.querySelector('#range-vol').oninput = (e) => {
    sounds.setVolume(parseFloat(e.target.value));
  };

  const btnScanDoctor = container.querySelector('#btn-scan-doctor');
  if (btnScanDoctor) {
    btnScanDoctor.onclick = async () => {
      sounds.play('ui.click');
      const out = container.querySelector('#doctor-profile-output');
      if (out) out.innerHTML = `<span style="color: var(--accent);">⟳ Scanning host machine via Rust SystemScanner...</span>`;
      try {
        const doc = await apiClient.getDoctor();
        sounds.play('verdict.green');
        const toolsHtml = (doc.installed_tools || [])
          .map(t => `<li style="margin-bottom: 2px;"><strong>${t.binary}</strong>: <code class="font-mono">${t.version_output || 'detected'}</code> (<span class="font-mono">${t.path}</span>)</li>`)
          .join('');
        if (out) {
          out.innerHTML = `
            <div style="background: rgba(0,0,0,0.04); border-radius: var(--radius-md); padding: 12px; margin-top: 6px;">
              <div><strong>OS:</strong> ${doc.os_name} (${doc.os_version}) · <strong>Arch:</strong> ${doc.arch}</div>
              <div><strong>GPU:</strong> ${doc.gpu_detected || 'Standard'}</div>
              <div style="margin-top: 8px; font-weight: 600;">Detected Tools (${(doc.installed_tools || []).length}):</div>
              <ul style="padding-left: 18px; margin: 4px 0 0 0;">${toolsHtml}</ul>
            </div>
          `;
        }
      } catch (err) {
        sounds.play('error');
        if (out) out.innerHTML = `<span style="color: var(--v-red);">Scan error: ${err.message}</span>`;
      }
    };
  }

  // AI Configuration Management
  const badgeEl = container.querySelector('#ai-key-status-badge');
  const providerSelect = container.querySelector('#select-ai-provider');
  const keyInput = container.querySelector('#input-ai-key');
  const keyHint = container.querySelector('#ai-key-hint');
  const modelInput = container.querySelector('#input-ai-model');
  const endpointInput = container.querySelector('#input-ai-endpoint');
  const btnToggleVis = container.querySelector('#btn-toggle-key-vis');
  const btnSaveKey = container.querySelector('#btn-save-ai-key');
  const btnClearKey = container.querySelector('#btn-clear-ai-key');
  const msgEl = container.querySelector('#ai-save-msg');

  const updateAiBadge = () => {
    const key = localStorage.getItem('setuppal_ai_api_key') || '';
    if (key.trim()) {
      const masked = key.length > 10 ? `${key.slice(0, 6)}...${key.slice(-4)}` : '••••••';
      badgeEl.innerHTML = `<span style="background: rgba(34, 197, 94, 0.15); color: #16a34a; font-size: 11px; font-weight: 600; padding: 4px 10px; border-radius: var(--radius-pill); border: 1px solid rgba(34, 197, 94, 0.3);">● Connected (${masked})</span>`;
    } else {
      badgeEl.innerHTML = `<span style="background: rgba(239, 68, 68, 0.1); color: var(--v-red); font-size: 11px; font-weight: 600; padding: 4px 10px; border-radius: var(--radius-pill); border: 1px solid rgba(239, 68, 68, 0.25);">○ Missing Key (Offline Recipes Only)</span>`;
    }
  };

  // Populate saved values (defaulting to NVIDIA NIM for nvapi keys)
  const savedKey = localStorage.getItem('setuppal_ai_api_key') || '';
  const savedProvider = localStorage.getItem('setuppal_ai_provider') || (savedKey.startsWith('sk-') ? 'openai' : 'nim');
  const savedModel = localStorage.getItem('setuppal_ai_model') || (savedProvider === 'nim' ? 'meta/llama-3.3-70b-instruct' : 'gpt-4o-mini');
  const savedEndpoint = localStorage.getItem('setuppal_ai_endpoint') || (savedProvider === 'nim' ? 'https://integrate.api.nvidia.com/v1/chat/completions' : 'https://api.openai.com/v1/chat/completions');

  if (providerSelect) providerSelect.value = savedProvider;
  if (keyInput) keyInput.value = savedKey;
  if (modelInput) modelInput.value = savedModel;
  if (endpointInput) endpointInput.value = savedEndpoint;
  if (keyHint) keyHint.textContent = savedProvider === 'nim' ? 'nvapi-...' : 'sk-...';
  updateAiBadge();

  // Wire model preset buttons
  container.querySelectorAll('.btn-model-preset').forEach(btn => {
    const targetModel = btn.getAttribute('data-model');
    if (modelInput && modelInput.value === targetModel) {
      btn.style.borderColor = 'var(--accent, #387EB8)';
      btn.style.fontWeight = '600';
    }
    btn.onclick = () => {
      if (modelInput) {
        modelInput.value = targetModel;
        sounds.play('ui.click');
        container.querySelectorAll('.btn-model-preset').forEach(b => {
          b.style.borderColor = 'var(--border)';
          b.style.fontWeight = 'normal';
        });
        btn.style.borderColor = 'var(--accent, #387EB8)';
        btn.style.fontWeight = '600';
      }
    };
  });

  if (keyInput) {
    keyInput.oninput = (e) => {
      const val = e.target.value.trim();
      if (val.startsWith('nvapi-')) {
        if (providerSelect) providerSelect.value = 'nim';
        if (modelInput && (!modelInput.value || modelInput.value.includes('gpt-'))) {
          modelInput.value = 'meta/llama-3.3-70b-instruct';
        }
        if (endpointInput) endpointInput.value = 'https://integrate.api.nvidia.com/v1/chat/completions';
        if (keyHint) keyHint.textContent = 'nvapi-...';
      } else if (val.startsWith('sk-')) {
        if (providerSelect) providerSelect.value = 'openai';
        if (modelInput && (!modelInput.value || modelInput.value.includes('llama') || modelInput.value.includes('nemotron') || modelInput.value.includes('mistral'))) {
          modelInput.value = 'gpt-4o-mini';
        }
        if (endpointInput && (!endpointInput.value || endpointInput.value.includes('nvidia'))) endpointInput.value = 'https://api.openai.com/v1/chat/completions';
        if (keyHint) keyHint.textContent = 'sk-...';
      }
    };
  }

  if (providerSelect) {
    providerSelect.onchange = (e) => {
      const prov = e.target.value;
      if (prov === 'nim') {
        if (!modelInput.value || modelInput.value.includes('gpt-')) modelInput.value = 'meta/llama-3.3-70b-instruct';
        if (!endpointInput.value || endpointInput.value.includes('openai.com')) endpointInput.value = 'https://integrate.api.nvidia.com/v1/chat/completions';
        if (keyHint) keyHint.textContent = 'nvapi-...';
      } else if (prov === 'openai') {
        if (!modelInput.value || modelInput.value.includes('llama') || modelInput.value.includes('nemotron')) modelInput.value = 'gpt-4o-mini';
        if (!endpointInput.value || endpointInput.value.includes('nvidia.com')) endpointInput.value = 'https://api.openai.com/v1/chat/completions';
        if (keyHint) keyHint.textContent = 'sk-...';
      } else {
        if (keyHint) keyHint.textContent = 'api-key';
      }
    };
  }

  if (btnToggleVis && keyInput) {
    btnToggleVis.onclick = () => {
      keyInput.type = keyInput.type === 'password' ? 'text' : 'password';
      btnToggleVis.textContent = keyInput.type === 'password' ? '👁️' : '🙈';
    };
  }

  if (btnSaveKey) {
    btnSaveKey.onclick = () => {
      const keyVal = (keyInput?.value || '').trim();
      const provVal = providerSelect?.value || 'openai';
      const modelVal = (modelInput?.value || '').trim();
      const endpointVal = (endpointInput?.value || '').trim();

      let finalProv = provVal;
      let finalModel = modelVal;
      let finalEndpoint = endpointVal;

      if (keyVal.startsWith('nvapi-')) {
        finalProv = 'nim';
        if (!finalEndpoint || finalEndpoint.includes('openai.com')) finalEndpoint = 'https://integrate.api.nvidia.com/v1/chat/completions';
        if (!finalModel || finalModel.includes('gpt-')) finalModel = 'meta/llama-3.3-70b-instruct';
      }

      if (keyVal) {
        localStorage.setItem('setuppal_ai_api_key', keyVal);
      } else {
        localStorage.removeItem('setuppal_ai_api_key');
      }
      localStorage.setItem('setuppal_ai_provider', finalProv);
      if (finalModel) localStorage.setItem('setuppal_ai_model', finalModel);
      if (finalEndpoint) localStorage.setItem('setuppal_ai_endpoint', finalEndpoint);

      sounds.play('task.done');
      updateAiBadge();
      if (msgEl) {
        msgEl.innerHTML = `<span style="color: #16a34a;">✓ AI configuration saved!</span>`;
        setTimeout(() => { if (msgEl) msgEl.innerHTML = ''; }, 3000);
      }
    };
  }

  const btnTestKey = container.querySelector('#btn-test-ai-key');
  if (btnTestKey) {
    btnTestKey.onclick = async () => {
      const keyVal = (keyInput?.value || '').trim() || localStorage.getItem('setuppal_ai_api_key') || '';
      if (!keyVal) {
        if (msgEl) msgEl.innerHTML = `<span style="color: var(--v-red);">Please enter an API key first.</span>`;
        return;
      }
      const provVal = providerSelect?.value || (keyVal.startsWith('nvapi-') ? 'nim' : 'openai');
      const modelVal = (modelInput?.value || '').trim();
      const endpointVal = (endpointInput?.value || '').trim();

      if (msgEl) msgEl.innerHTML = `<span style="color: var(--accent);">⟳ Testing key with ${provVal === 'nim' ? 'NVIDIA NIM' : 'AI provider'}...</span>`;
      sounds.play('ui.click');

      try {
        const res = await apiClient.validateKey({
          apiKey: keyVal,
          provider: provVal,
          model: modelVal || undefined,
          endpoint: endpointVal || undefined,
        });

        if (res.valid) {
          sounds.play('verdict.green');
          if (msgEl) msgEl.innerHTML = `<span style="color: #16a34a;">✓ ${res.message}</span>`;
        } else {
          sounds.play('verdict.red');
          if (msgEl) msgEl.innerHTML = `<span style="color: var(--v-red);">✗ ${res.message}</span>`;
        }
      } catch (err) {
        sounds.play('error');
        if (msgEl) msgEl.innerHTML = `<span style="color: var(--v-red);">Error: ${err.message}</span>`;
      }
    };
  }

  if (btnClearKey) {
    btnClearKey.onclick = () => {
      localStorage.removeItem('setuppal_ai_api_key');
      if (keyInput) keyInput.value = '';
      sounds.play('ui.click');
      updateAiBadge();
      if (msgEl) {
        msgEl.innerHTML = `<span style="color: var(--ink-soft);">API key cleared. Offline recipes active.</span>`;
        setTimeout(() => { if (msgEl) msgEl.innerHTML = ''; }, 3000);
      }
    };
  }
}
