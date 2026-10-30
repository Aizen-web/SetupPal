/**
 * SetupPal: Real-time Zero-Trust Backend API Client
 * Connects the UI to the embedded Axum backend. The address and per-launch bearer token come from
 * the Tauri shell (`backend_info` command), and every request, including event streams, carries the token.
 */

import { invoke } from '@tauri-apps/api/core';
import { chatPrefsPayload, executePrefsPayload } from '../state/prefs.js';

let backendPromise = null;

/** Asks the shell once for `{ apiBase, token }` and caches it for the life of the window. */
export function getBackend() {
  if (!backendPromise) {
    backendPromise = invoke('backend_info')
      .then(info => ({ apiBase: info.api_base, token: info.token }))
      .catch(err => {
        backendPromise = null;
        throw new Error(
          `Could not reach the SetupPal shell (${err?.message || err}). Start the app with "npm --prefix crates/gui run app".`,
        );
      });
  }
  return backendPromise;
}

/** fetch() against the backend with the bearer token attached. `path` starts with "/api". */
async function authFetch(path, init = {}) {
  const { apiBase, token } = await getBackend();
  const headers = new Headers(init.headers || {});
  headers.set('Authorization', `Bearer ${token}`);
  return fetch(`${apiBase}${path}`, { ...init, headers });
}

function jsonInit(method, body, { headers = {}, ...extra } = {}) {
  return {
    method,
    headers: { 'Content-Type': 'application/json', ...headers },
    body: JSON.stringify(body),
    ...extra,
  };
}

/**
 * Reads a Server-Sent Events response and calls `onBlock({ event, data })` for each event.
 * (EventSource cannot send an Authorization header, so streams are read with fetch.)
 */
export async function readEventStream(response, onBlock) {
  if (!response.body) throw new Error('The backend did not open a streaming response.');
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  const dispatch = (block) => {
    const lines = block.split(/\r?\n/);
    const event = lines.find(line => line.startsWith('event:'))?.slice(6).trim() || 'message';
    const data = lines.filter(line => line.startsWith('data:'))
      .map(line => line.slice(5).replace(/^ /, '')).join('\n');
    if (data) onBlock({ event, data });
  };
  try {
    while (true) {
      const { value, done } = await reader.read();
      buffer += decoder.decode(value || new Uint8Array(), { stream: !done });
      const blocks = buffer.split(/\r?\n\r?\n/);
      buffer = blocks.pop() || '';
      for (const block of blocks) dispatch(block);
      if (done) break;
    }
    if (buffer.trim()) dispatch(buffer);
  } finally {
    reader.releaseLock();
  }
}

/** Reconnect delay: 0.5s, 1s, 2s ... capped at 10s. */
export function reconnectDelay(attempt) {
  return Math.min(500 * 2 ** attempt, 10000);
}

class ApiClient {
  constructor() {
    this.eventAbort = null;
    this.isConnected = false;
    this.lastHealth = null;
  }

  async checkHealth(timeoutMs = 1500) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), timeoutMs);
    try {
      const res = await authFetch('/api/health', { signal: controller.signal });
      clearTimeout(timeout);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      this.isConnected = true;
      this.lastHealth = data;
      return data;
    } catch (err) {
      clearTimeout(timeout);
      this.isConnected = false;
      return null;
    }
  }

  async getDoctor() {
    const res = await authFetch('/api/doctor');
    if (!res.ok) throw new Error(`Doctor scan failed: ${res.statusText}`);
    return await res.json();
  }

  async getRecipes() {
    const res = await authFetch('/api/recipes');
    if (!res.ok) throw new Error(`Fetch recipes failed: ${res.statusText}`);
    return await res.json();
  }

  /** `provider` is `nebius`, `tavily`, or `github`. An empty `apiKey` tests the saved key. */
  async validateKey({ apiKey = null, provider = 'nebius' } = {}) {
    try {
      const res = await authFetch('/api/test-key', jsonInit('POST', {
        api_key: (apiKey || '').trim(),
        provider,
      }));

      if (!res.ok) {
        const text = await res.text();
        return { valid: false, message: `Server error (${res.status}): ${text}` };
      }

      return await res.json();
    } catch (err) {
      return { valid: false, message: `Could not reach backend: ${err.message}` };
    }
  }

  /** Saves a key to the OS keychain. The value is sent once and never read back. */
  async saveKey(name, value) {
    const res = await authFetch('/api/keys', jsonInit('POST', { name, value }));
    if (!res.ok) {
      const text = await res.text();
      throw new Error(text || `Could not save the key (${res.status}).`);
    }
    return await res.json();
  }

  async deleteKey(name) {
    const res = await authFetch(`/api/keys/${encodeURIComponent(name)}`, { method: 'DELETE' });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(text || `Could not remove the key (${res.status}).`);
    }
    return await res.json();
  }

  /** `{ configured: { name: bool }, in_keychain: { name: bool } }`. Never contains key material. */
  async getKeyStatus() {
    const res = await authFetch('/api/keys/status');
    if (!res.ok) throw new Error(`Key status failed (${res.status}).`);
    return await res.json();
  }

  /** Allowlisted Nemotron catalog + currently selected model. */
  async getModel() {
    const res = await authFetch('/api/model');
    if (!res.ok) throw new Error(`Model status failed (${res.status}).`);
    return await res.json();
  }

  /**
   * Persists a Nemotron model choice (`lightning` | `super` | `ultra`).
   * Arbitrary IDs are rejected by the backend; there is no silent fallback.
   */
  async setModel(model) {
    const res = await authFetch('/api/model', jsonInit('PUT', { model }));
    if (!res.ok) {
      const text = await res.text();
      throw new Error(text || `Could not set model (${res.status}).`);
    }
    const data = await res.json();
    if (this.lastHealth) {
      this.lastHealth = {
        ...this.lastHealth,
        model: data.model,
        model_id: data.model_id,
        model_name: data.model_name,
        models: data.models,
      };
    }
    return data;
  }

  async sendChatStream(message, history = [], ...rest) {
    // Callers may still pass the retired pet name before the options object.
    const { signal, onEvent } = rest.find(arg => arg && typeof arg === 'object') || {};
    const response = await authFetch('/api/chat/stream', jsonInit('POST', {
      message,
      history,
      ...chatPrefsPayload(),
    }, { signal, headers: { Accept: 'text/event-stream' } }));

    if (!response.ok) {
      const message = await response.text();
      throw new Error(message || `Chat failed (${response.status}).`);
    }

    await readEventStream(response, ({ event, data }) => {
      let payload;
      try {
        payload = JSON.parse(data);
      } catch {
        throw new Error('The backend sent an invalid streaming event.');
      }
      onEvent?.({ type: event, ...payload });
    });
  }

  async createPlan(goal, { untrustedReadme = null, context = null, verifyManual = false } = {}) {
    const res = await authFetch('/api/plan', jsonInit('POST', {
      goal,
      untrusted_readme: untrustedReadme,
      context: context || null,
      verify_manual: verifyManual,
    }));
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Plan creation failed (${res.status}): ${text}`);
    }
    return await res.json();
  }

  async executePlan({
    runId = null,
    planId = null,
    dryRun = false,
    autoApproveGreen = null,
    approvedStepIds = [],
    allowYellowSteps = [],
    startAtStepId = null,
    projectDir = null,
    trustedCategories = [],
  } = {}) {
    const res = await authFetch('/api/execute', jsonInit('POST', {
      run_id: runId,
      plan_id: planId,
      dry_run: dryRun,
      auto_approve_green: autoApproveGreen,
      approved_step_ids: approvedStepIds,
      allow_yellow_steps: allowYellowSteps,
      start_at_step_id: startAtStepId,
      project_dir: projectDir,
      trusted_categories: trustedCategories,
      ...executePrefsPayload(),
    }));
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Plan execution failed (${res.status}): ${text}`);
    }
    return await res.json();
  }

  async cancelExecute(runId) {
    const res = await authFetch('/api/execute/cancel', jsonInit('POST', { run_id: runId }));
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Cancel failed (${res.status}): ${text}`);
    }
    return await res.json();
  }

  async getHistory() {
    const res = await authFetch('/api/history');
    if (!res.ok) throw new Error(`Fetch history failed: ${res.statusText}`);
    return await res.json();
  }

  async undo(txId = null) {
    const res = await authFetch('/api/undo', jsonInit('POST', { tx_id: txId }));
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Rollback failed (${res.status}): ${text}`);
    }
    return await res.json();
  }

  /**
   * Streams backend events with fetch so the bearer token can be sent. Reconnects with backoff
   * until `disconnectEvents()` is called.
   */
  connectEvents(onEvent, onError) {
    this.disconnectEvents();
    const controller = new AbortController();
    this.eventAbort = controller;

    const run = async () => {
      let attempt = 0;
      while (!controller.signal.aborted) {
        try {
          const response = await authFetch('/api/events', {
            headers: { Accept: 'text/event-stream' },
            signal: controller.signal,
          });
          if (!response.ok) throw new Error(`Event stream failed (${response.status}).`);
          attempt = 0;
          await readEventStream(response, ({ data }) => {
            try {
              onEvent?.(JSON.parse(data));
            } catch (e) {
              console.error('Error parsing SSE event data:', e);
            }
          });
          // Stream ended cleanly; fall through to reconnect.
        } catch (err) {
          if (controller.signal.aborted) return;
          onError?.(err);
        }
        if (controller.signal.aborted) return;
        await new Promise(resolve => setTimeout(resolve, reconnectDelay(attempt)));
        attempt += 1;
      }
    };
    run();
  }

  disconnectEvents() {
    if (this.eventAbort) {
      this.eventAbort.abort();
      this.eventAbort = null;
    }
  }
}

export const apiClient = new ApiClient();
