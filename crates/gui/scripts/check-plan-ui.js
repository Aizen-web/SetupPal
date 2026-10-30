#!/usr/bin/env node
/**
 * Focused checks for plan-card helpers (R2 grounding, R11 labels, R23 versions).
 * Run: node ./scripts/check-plan-ui.js
 */
// bridge.js pulls the store (browser APIs). Stub the minimum for Node.
const memory = new Map();
globalThis.localStorage = {
  getItem: (k) => (memory.has(k) ? memory.get(k) : null),
  setItem: (k, v) => memory.set(k, String(v)),
  removeItem: (k) => memory.delete(k),
};
globalThis.window = globalThis;
globalThis.document = {
  documentElement: { dataset: {}, style: { setProperty() {} } },
};

const { intentGroundedInUserText, installStepLabel, toActivePlan } = await import('../src/api/bridge.js');

const failures = [];
const expect = (name, ok, detail = '') => {
  if (!ok) failures.push(`${name}${detail ? ` — ${detail}` : ''}`);
};

// R2: intents must be grounded in user text
expect(
  'grounded when user named the app',
  intentGroundedInUserText(
    { kind: 'install', goal: 'vlc' },
    [{ sender: 'user', text: 'install vlc please' }],
  ),
);
expect(
  'ungrounded assistant-only suggestion is rejected',
  !intentGroundedInUserText(
    { kind: 'install', goal: 'vscode' },
    [{ sender: 'user', text: 'install rust' }, { sender: 'assistant', text: 'Want VS Code too?' }],
  ),
);

// R23: versioned install labels
expect(
  'winget label with version',
  installStepLabel('LMMS.LMMS', 'winget', '1.2.2') === 'Install LMMS 1.2.2 (winget)',
);
expect(
  'pip label without version',
  installStepLabel('requests', 'pypi', null) === 'Install requests (pip)',
);

const planData = {
  plan: {
    plan_id: 'p1',
    goal: 'LMMS',
    summary: 'Installs LMMS',
    source: { resolved: { query: 'LMMS' } },
    steps: [{
      id: 'step-1',
      description: 'Install LMMS (winget)',
      step: {
        action: 'install_package',
        params: { package_id: 'LMMS.LMMS', ecosystem: 'winget', version: '1.2.2' },
      },
    }],
  },
  safety: {
    overall_rating: 'green',
    notes: ['OSV.dev does not cover winget packages; those steps rely on winget metadata and policy checks.'],
    step_reports: [{ step_id: 'step-1', rating: 'green', reasons: [] }],
  },
  step_metadata: [{ step_id: 'step-1', resolved_version: '1.2.2' }],
};

const active = toActivePlan(planData);
expect('toActivePlan builds a card', Boolean(active?.tasks?.length === 1));
expect(
  'step title includes version',
  active?.tasks?.[0]?.title === 'Install LMMS 1.2.2 (winget)',
  active?.tasks?.[0]?.title,
);
expect(
  'OSV note appears once on subtitle',
  String(active?.subtitle || '').includes('OSV.dev does not cover winget'),
);

if (failures.length) {
  console.error(`Plan UI check failed (${failures.length}):`);
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}
console.log('Plan UI check passed: grounding, version labels, and OSV note.');
