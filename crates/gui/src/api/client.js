/**
 * SetupPal: Real-time Zero-Trust Backend API Client
 * Connects modern UI frontend to installer_core Axum backend & SSE event stream.
 */

import { chatPrefsPayload } from '../state/prefs.js';

const API_BASE = '/api';

class ApiClient {
  constructor() {
    this.eventSource = null;
    this.isConnected = false;
    this.lastHealth = null;
  }

  async checkHealth(timeoutMs = 1500) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), timeoutMs);
    try {
      const res = await fetch(`${API_BASE}/health`, { signal: controller.signal });
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
    const res = await fetch(`${API_BASE}/doctor`);
    if (!res.ok) throw new Error(`Doctor scan failed: ${res.statusText}`);
    return await res.json();
  }

  async getRecipes() {
    const res = await fetch(`${API_BASE}/recipes`);
    if (!res.ok) throw new Error(`Fetch recipes failed: ${res.statusText}`);
    return await res.json();
  }

  async validateKey({ apiKey = null } = {}) {
    const key = apiKey || localStorage.getItem('setuppal_ai_api_key');
    if (!key || !key.trim()) {
      return { valid: false, message: 'No API key provided.' };
    }

    try {
      const res = await fetch(`${API_BASE}/test-key`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          api_key: key.trim(),
        }),
      });

      if (!res.ok) {
        const text = await res.text();
        return { valid: false, message: `Server error (${res.status}): ${text}` };
      }
      return await res.json();
    } catch (err) {
      return { valid: false, message: `Could not reach backend: ${err.message}` };
    }
  }

  async sendChat(message, history = [], pet = 'ghost') {
    const apiKey = localStorage.getItem('setuppal_ai_api_key') || null;

    const res = await fetch(`${API_BASE}/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        message,
        history,
        pet,
        api_key: apiKey,
        ...chatPrefsPayload(),
      }),
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Chat failed (${res.status}): ${text}`);
    }
    return await res.json();
  }

  async createPlan(goal, untrustedReadme = null) {
    const res = await fetch(`${API_BASE}/plan`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ goal, untrusted_readme: untrustedReadme }),
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Plan creation failed (${res.status}): ${text}`);
    }
    return await res.json();
  }

  async executePlan({ planId = null, dryRun = false, autoApproveGreen = true, allowYellow = false, customPlan = null, trustedCategories = [] } = {}) {
    const res = await fetch(`${API_BASE}/execute`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        plan_id: planId,
        dry_run: dryRun,
        auto_approve_green: autoApproveGreen,
        allow_yellow: allowYellow,
        custom_plan: customPlan,
        trusted_categories: trustedCategories,
      }),
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Plan execution failed (${res.status}): ${text}`);
    }
    return await res.json();
  }

  async getHistory() {
    const res = await fetch(`${API_BASE}/history`);
    if (!res.ok) throw new Error(`Fetch history failed: ${res.statusText}`);
    return await res.json();
  }

  async undo(txId = null) {
    const res = await fetch(`${API_BASE}/undo`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tx_id: txId }),
    });
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Rollback failed (${res.status}): ${text}`);
    }
    return await res.json();
  }

  connectEvents(onEvent, onError) {
    if (this.eventSource) {
      this.eventSource.close();
    }

    try {
      this.eventSource = new EventSource(`${API_BASE}/events`);

      this.eventSource.onmessage = (event) => {
        try {
          const parsed = JSON.parse(event.data);
          if (onEvent) onEvent(parsed);
        } catch (e) {
          console.error("Error parsing SSE event data:", e, event.data);
        }
      };

      this.eventSource.onerror = (err) => {
        if (onError) onError(err);
      };
    } catch (e) {
      console.warn("Could not establish EventSource:", e);
    }
  }

  disconnectEvents() {
    if (this.eventSource) {
      this.eventSource.close();
      this.eventSource = null;
    }
  }
}

export const apiClient = new ApiClient();
