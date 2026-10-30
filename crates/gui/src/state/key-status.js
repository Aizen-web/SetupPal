/**
 * SetupPal: which keys are set, as reported by the backend (booleans only, never key material).
 */

import { apiClient } from '../api/client.js';
import { store } from './store.js';

let lastStatus = null;

export function currentKeyStatus() {
  return lastStatus;
}

/** Fetches key status and mirrors the optional-service flags into the store. */
export async function refreshKeyStatus() {
  const status = await apiClient.getKeyStatus();
  lastStatus = status;
  const configured = status.configured || {};
  store.setState(s => ({
    ...s,
    accounts: {
      ...s.accounts,
      nebiusConnected: Boolean(configured.nebius_api_key),
      tavilyConnected: Boolean(configured.tavily_api_key),
      githubConnected: Boolean(configured.github_token),
    },
  }));
  return status;
}
