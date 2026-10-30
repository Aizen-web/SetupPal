// Drives the running SetupPal webview via CDP like a user would.
// Usage: node drive.mjs <command> [arg]
//  new                 -> click New chat
//  say "<text>"        -> type + send, wait until the reply settles, print a summary
//  eval "<js>"         -> evaluate JS
//  approve             -> click the approve button on the plan card, wait for run to finish
//  snap                -> print current page snapshot JSON
//  shot <file>         -> screenshot
//  page <name>         -> click nav-<name>
//  monitor-start       -> enable Runtime/Log listeners (prints events as JSON lines)
//  assert-plan <ids>   -> comma-separated expected package IDs; exit 1 on mismatch
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

const DEFAULT_PORT = 9333;

const SNAP = `(() => {
  const t = s => [...document.querySelectorAll(s)].map(e => e.innerText.trim());
  const msgs = [...document.querySelectorAll('#chat-thread-stream > *')].map(e => {
    const body = e.querySelector('.chat-bubble-body, .user-text');
    return (body ? body.innerText : e.innerText).trim();
  }).filter(Boolean);
  const plan = document.querySelector('#plan-card-title');
  const manual = document.querySelector('#manual-guide-title');
  const stepRows = [...document.querySelectorAll('.step-row')].map(e => e.innerText.replace(/\\n/g, ' | '));
  const pkgIds = [...document.querySelectorAll('[data-package-id], .step-row [data-pkg], .plan-step-id')]
    .map(e => e.getAttribute('data-package-id') || e.getAttribute('data-pkg') || e.innerText.trim())
    .filter(Boolean);
  // Fallback: scrape package-looking tokens from the plan card body.
  const planText = plan ? (plan.closest('section,article,div.plan-card,div')?.parentElement?.innerText || '') : '';
  const idHits = [...planText.matchAll(/\\b([A-Za-z0-9][A-Za-z0-9+_.-]*\\.[A-Za-z0-9][A-Za-z0-9+_.-]*)\\b/g)].map(m => m[1]);
  const npmPy = [...planText.matchAll(/\\b(?:npm|pip|pypi)\\b[^\\n]*\\b([a-z0-9_.-]+)\\b/gi)].map(m => m[1].toLowerCase());
  return JSON.stringify({
    lastMsgs: msgs.slice(-3),
    planTitle: plan?.innerText || null,
    planCard: planText ? planText.slice(0, 1800) : null,
    manualCard: manual ? manual.closest('section,article,div')?.parentElement?.innerText.slice(0, 1800) : null,
    manualTitle: manual?.innerText || null,
    stepRows,
    packageIds: [...new Set([...pkgIds, ...idHits, ...npmPy])],
    approveBtn: document.querySelector('#btn-approve-plan')?.innerText || null,
    approveDisabled: document.querySelector('#btn-approve-plan')?.disabled ?? null,
    notice: document.querySelector('#composer-notice')?.innerText || '',
    terminal: t('#terminal-log-output > *').slice(-40),
  });
})()`;

export class CdpSession {
  constructor(ws, page) {
    this.ws = ws;
    this.page = page;
    this.id = 0;
    this.pend = new Map();
    this.events = [];
    this._onMessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id != null && this.pend.has(m.id)) {
        this.pend.get(m.id)(m);
        this.pend.delete(m.id);
        return;
      }
      if (m.method) this.events.push({ at: Date.now(), method: m.method, params: m.params });
    };
    this.ws.addEventListener('message', this._onMessage);
  }

  send(method, params = {}) {
    return new Promise((res) => {
      const i = ++this.id;
      this.pend.set(i, res);
      this.ws.send(JSON.stringify({ id: i, method, params }));
    });
  }

  async ev(expression) {
    const m = await this.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (m.result?.exceptionDetails) {
      return 'EXC ' + (m.result.exceptionDetails.exception?.description || m.result.exceptionDetails.text);
    }
    return m.result?.result?.value;
  }

  async snap() {
    const raw = await this.ev(SNAP);
    if (typeof raw !== 'string' || raw.startsWith('EXC')) {
      throw new Error(`snap failed: ${raw}`);
    }
    return JSON.parse(raw);
  }

  async newChat() {
    return this.ev(`document.querySelector('#btn-new-chat')?.click(), 'ok'`);
  }

  async say(text, { timeoutMs = 150000, settle = 4 } = {}) {
    const payload = JSON.stringify(text);
    await this.ev(`(() => { const i = document.querySelector('#chat-input'); i.focus(); const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set; set.call(i, ${payload}); i.dispatchEvent(new Event('input',{bubbles:true})); document.querySelector('#btn-chat-send').click(); })()`);
    const t0 = Date.now();
    let last = '';
    let stable = 0;
    let snap = null;
    while (Date.now() - t0 < timeoutMs) {
      await sleep(1500);
      snap = await this.snap();
      const s = JSON.stringify(snap);
      if (s === last) stable++;
      else {
        stable = 0;
        last = s;
      }
      if (stable >= settle && Date.now() - t0 > 5000) break;
    }
    return { elapsedMs: Date.now() - t0, snap };
  }

  async approve({ timeoutMs = 300000 } = {}) {
    await this.ev(`document.querySelector('#btn-approve-plan')?.click()`);
    const t0 = Date.now();
    let last = '';
    let stable = 0;
    let snap = null;
    while (Date.now() - t0 < timeoutMs) {
      await sleep(2000);
      snap = await this.snap();
      const s = JSON.stringify(snap);
      if (s === last) stable++;
      else {
        stable = 0;
        last = s;
      }
      if (stable >= 6) break;
    }
    return { elapsedMs: Date.now() - t0, snap };
  }

  /** Enable Runtime exceptions + Log + page console; collect unhandledrejection / CSP-like noise. */
  async enableMonitoring() {
    await this.send('Runtime.enable');
    await this.send('Log.enable');
    await this.send('Page.enable');
    await this.send('Console.enable').catch(() => null);
    // Hook page-level rejection + securitypolicyviolation when possible.
    await this.ev(`(() => {
      if (window.__setuppalCorpusHooks) return 'already';
      window.__setuppalCorpusHooks = true;
      window.__setuppalCorpusEvents = [];
      window.addEventListener('unhandledrejection', (e) => {
        window.__setuppalCorpusEvents.push({ type: 'unhandledrejection', reason: String(e.reason) });
      });
      window.addEventListener('securitypolicyviolation', (e) => {
        window.__setuppalCorpusEvents.push({
          type: 'csp',
          violatedDirective: e.violatedDirective,
          blockedURI: e.blockedURI,
          effectiveDirective: e.effectiveDirective,
        });
      });
      return 'ok';
    })()`);
  }

  async pageEvents() {
    const raw = await this.ev(`JSON.stringify(window.__setuppalCorpusEvents || [])`);
    try {
      return JSON.parse(raw || '[]');
    } catch {
      return [];
    }
  }

  consoleProblems() {
    const problems = [];
    for (const ev of this.events) {
      if (ev.method === 'Runtime.exceptionThrown') {
        const desc = ev.params?.exceptionDetails?.exception?.description
          || ev.params?.exceptionDetails?.text
          || 'exception';
        problems.push({ kind: 'exception', detail: desc });
      }
      if (ev.method === 'Log.entryAdded') {
        const entry = ev.params?.entry;
        const text = entry?.text || '';
        if (/content security policy|csp|refused to/i.test(text)) {
          problems.push({ kind: 'csp', detail: text });
        }
        if (entry?.level === 'error' && /unhandledrejection|unhandled/i.test(text)) {
          problems.push({ kind: 'unhandledrejection', detail: text });
        }
      }
      if (ev.method === 'Runtime.consoleAPICalled' && ev.params?.type === 'error') {
        const text = (ev.params.args || []).map((a) => a.value ?? a.description ?? '').join(' ');
        if (/unhandledrejection|content security policy|csp/i.test(text)) {
          problems.push({ kind: 'console-error', detail: text });
        }
      }
    }
    return problems;
  }

  close() {
    try {
      this.ws.removeEventListener('message', this._onMessage);
      this.ws.close();
    } catch {
      /* ignore */
    }
  }
}

export function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

export async function connectCdp({ port = DEFAULT_PORT } = {}) {
  let list;
  try {
    list = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
  } catch (err) {
    const e = new Error(
      `SetupPal CDP not reachable on 127.0.0.1:${port}. Start the app with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=${port}. (${err.message})`,
    );
    e.code = 'CDP_UNAVAILABLE';
    throw e;
  }
  const page = list.find((t) => t.type === 'page');
  if (!page?.webSocketDebuggerUrl) {
    const e = new Error(`No debuggable page found on port ${port}.`);
    e.code = 'CDP_NO_PAGE';
    throw e;
  }
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((r, j) => {
    ws.onopen = r;
    ws.onerror = () => j(new Error('WebSocket connect failed'));
  });
  return new CdpSession(ws, page);
}

/** Extract install package IDs from a snap produced by SNAP. */
export function extractPlanIds(snap) {
  const ids = new Set();
  for (const id of snap.packageIds || []) {
    if (id && typeof id === 'string') ids.add(id);
  }
  const blob = [snap.planCard, ...(snap.stepRows || [])].filter(Boolean).join('\n');
  for (const m of blob.matchAll(/\b([A-Za-z0-9][A-Za-z0-9+_.-]*\.[A-Za-z0-9][A-Za-z0-9+_.-]*)\b/g)) {
    ids.add(m[1]);
  }
  // Common registry names called out on the card.
  for (const m of blob.matchAll(/\b(?:Install|install)\s+([a-z0-9_@/.-]+)\s*(?:\(|via|from)?\s*(?:npm|pip|pypi)?/gi)) {
    const name = m[1].replace(/[,:]$/, '');
    if (name && !name.includes(' ') && name.length < 64) ids.add(name);
  }
  return [...ids];
}

/**
 * Assert plan install IDs match expected set.
 * Returns { ok, missing, unexpected, found }.
 */
export function assertPlanSteps(snap, expectedIds, { allowExtra = false } = {}) {
  const found = extractPlanIds(snap);
  const want = new Set(expectedIds);
  const have = new Set(found);
  const missing = [...want].filter((id) => ![...have].some((h) => h === id || h.includes(id) || id.includes(h)));
  const unexpected = allowExtra
    ? []
    : [...have].filter((id) => ![...want].some((w) => w === id || id.includes(w) || w.includes(id)));
  return {
    ok: missing.length === 0 && unexpected.length === 0,
    missing,
    unexpected,
    found,
  };
}

export function assertManualCard(snap, itemContains = []) {
  const text = `${snap.manualTitle || ''}\n${snap.manualCard || ''}`.toLowerCase();
  if (!snap.manualCard && !snap.manualTitle) {
    return { ok: false, reason: 'no manual card' };
  }
  if (itemContains.length === 0) return { ok: true };
  const hit = itemContains.some((needle) => text.includes(String(needle).toLowerCase()));
  return hit ? { ok: true } : { ok: false, reason: `manual card missing any of: ${itemContains.join(', ')}` };
}

export function assertClarify(snap) {
  const msgs = (snap.lastMsgs || []).join('\n');
  const asks = /\?/.test(msgs);
  const hasPlan = Boolean(snap.planCard && snap.approveBtn);
  const hasManual = Boolean(snap.manualCard);
  if (hasPlan) return { ok: false, reason: 'expected clarify but plan card appeared' };
  if (hasManual) return { ok: false, reason: 'expected clarify but manual card appeared' };
  if (!asks) return { ok: false, reason: 'expected a clarifying question in the reply' };
  return { ok: true };
}

export function assertAlreadyInstalled(snap, ids = []) {
  const blob = `${snap.planCard || ''}\n${(snap.lastMsgs || []).join('\n')}\n${snap.notice || ''}`.toLowerCase();
  const saysInstalled = /already installed|installed \(v|is already on/.test(blob);
  if (!saysInstalled) return { ok: false, reason: 'no already-installed signal' };
  // Must not offer Approve for a reinstall of those ids.
  if (snap.approveBtn && /install/i.test(snap.planCard || '')) {
    const found = extractPlanIds(snap);
    const reinstall = ids.filter((id) => found.some((f) => f === id || f.includes(id)));
    if (reinstall.length) return { ok: false, reason: `plan still installs ${reinstall.join(', ')}` };
  }
  return { ok: true };
}

export function assertConfirm(snap, { typed, suggestion } = {}) {
  const blob = `${snap.planCard || ''}\n${(snap.lastMsgs || []).join('\n')}\n${snap.notice || ''}`.toLowerCase();
  const asks = /\?|did you mean|mean|confirm|instead/.test(blob);
  if (!asks) return { ok: false, reason: 'expected confirmation prompt' };
  if (suggestion && !blob.includes(String(suggestion).toLowerCase())) {
    return { ok: false, reason: `suggestion "${suggestion}" not mentioned` };
  }
  if (typed && !blob.includes(String(typed).toLowerCase()) && !asks) {
    return { ok: false, reason: `typed name "${typed}" not mentioned` };
  }
  // Must not silently install the suggestion without confirm UX.
  const found = extractPlanIds(snap);
  if (suggestion && found.includes(suggestion) && snap.approveBtn && !asks) {
    return { ok: false, reason: 'planned suggestion without confirmation' };
  }
  return { ok: true };
}

async function runCli() {
  const [cmd, arg, arg2] = process.argv.slice(2);
  let session;
  try {
    session = await connectCdp();
  } catch (err) {
    console.error(err.message);
    process.exit(err.code === 'CDP_UNAVAILABLE' || err.code === 'CDP_NO_PAGE' ? 2 : 1);
  }

  try {
    if (cmd === 'new') {
      console.log(await session.newChat());
      process.exit(0);
    }
    if (cmd === 'say') {
      const { elapsedMs, snap } = await session.say(arg);
      console.log(`elapsed ${elapsedMs / 1000}s`);
      console.log(JSON.stringify(snap, null, 1));
      process.exit(0);
    }
    if (cmd === 'approve') {
      const timeoutMs = (Number(arg) || 300) * 1000;
      const { elapsedMs, snap } = await session.approve({ timeoutMs });
      console.log(`elapsed ${elapsedMs / 1000}s`);
      console.log(JSON.stringify(snap, null, 1));
      process.exit(0);
    }
    if (cmd === 'snap') {
      console.log(JSON.stringify(await session.snap(), null, 1));
      process.exit(0);
    }
    if (cmd === 'eval') {
      console.log(JSON.stringify(await session.ev(arg), null, 1));
      process.exit(0);
    }
    if (cmd === 'page') {
      console.log(await session.ev(`document.querySelector('#nav-${arg}')?.click(), 'ok'`));
      process.exit(0);
    }
    if (cmd === 'shot') {
      const m = await session.send('Page.captureScreenshot', { format: 'png' });
      writeFileSync(arg, Buffer.from(m.result.data, 'base64'));
      console.log('saved', arg);
      process.exit(0);
    }
    if (cmd === 'monitor-start') {
      await session.enableMonitoring();
      console.log(JSON.stringify({ ok: true, monitoring: true }));
      // Keep process alive briefly so callers can attach — but CLI exits; live runner uses API.
      process.exit(0);
    }
    if (cmd === 'assert-plan') {
      const expected = String(arg || '')
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);
      const snap = await session.snap();
      const result = assertPlanSteps(snap, expected, { allowExtra: arg2 === '--allow-extra' });
      console.log(JSON.stringify({ ...result, snap: { planTitle: snap.planTitle, stepRows: snap.stepRows } }, null, 2));
      process.exit(result.ok ? 0 : 1);
    }
    console.error(`Unknown command: ${cmd || '(none)'}`);
    process.exit(1);
  } finally {
    session.close();
  }
}

const entry = process.argv[1] ? resolve(process.argv[1]) : '';
const self = fileURLToPath(import.meta.url);
const isMain = entry && self.toLowerCase() === entry.toLowerCase();
if (isMain) {
  await runCli();
}
