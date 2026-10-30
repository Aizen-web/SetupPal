// Drives the running SetupPal webview via CDP like a user would.
// Usage: node drive.mjs <command> [arg]
//  new                 -> click New chat
//  say "<text>"        -> type + send, wait until the reply settles, print a summary
//  eval "<js>"         -> evaluate JS
//  approve             -> click the approve button on the plan card, wait for run to finish
//  shot <file>         -> screenshot
//  page <name>         -> click nav-<name>
import { writeFileSync } from 'node:fs';
const [cmd, arg, arg2] = process.argv.slice(2);
const list = await (await fetch('http://127.0.0.1:9333/json')).json();
const page = list.find(t => t.type === 'page');
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise(r => (ws.onopen = r));
let id = 0; const pend = new Map();
ws.onmessage = e => { const m = JSON.parse(e.data); if (pend.has(m.id)) { pend.get(m.id)(m); pend.delete(m.id); } };
const send = (method, params = {}) => new Promise(res => { const i = ++id; pend.set(i, res); ws.send(JSON.stringify({ id: i, method, params })); });
const ev = async (expression) => {
  const m = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (m.result.exceptionDetails) return 'EXC ' + (m.result.exceptionDetails.exception?.description || m.result.exceptionDetails.text);
  return m.result.result.value;
};
const sleep = ms => new Promise(r => setTimeout(r, ms));

const SNAP = `(() => {
  const t = s => [...document.querySelectorAll(s)].map(e => e.innerText.trim());
  const msgs = [...document.querySelectorAll('#chat-thread-stream > *')].map(e => e.innerText.trim()).filter(Boolean);
  const plan = document.querySelector('#plan-card-title');
  const manual = document.querySelector('#manual-guide-title');
  return JSON.stringify({
    lastMsgs: msgs.slice(-3),
    planTitle: plan?.innerText || null,
    planCard: plan ? plan.closest('section,article,div.plan-card,div')?.parentElement?.innerText.slice(0, 1800) : null,
    manualCard: manual ? manual.closest('section,article,div')?.parentElement?.innerText.slice(0, 1800) : null,
    approveBtn: document.querySelector('#btn-approve-plan')?.innerText || null,
    approveDisabled: document.querySelector('#btn-approve-plan')?.disabled ?? null,
    notice: document.querySelector('#composer-notice')?.innerText || '',
    terminal: t('#terminal-log-output > *').slice(-40),
  });
})()`;

if (cmd === 'new') {
  console.log(await ev(`document.querySelector('#btn-new-chat')?.click(), 'ok'`));
} else if (cmd === 'say') {
  const text = JSON.stringify(arg);
  const before = await ev(`document.querySelectorAll('#chat-thread-stream > *').length`);
  await ev(`(() => { const i = document.querySelector('#chat-input'); i.focus(); const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set; set.call(i, ${text}); i.dispatchEvent(new Event('input',{bubbles:true})); document.querySelector('#btn-chat-send').click(); })()`);
  const t0 = Date.now(); let last = '', stable = 0;
  while (Date.now() - t0 < 150000) {
    await sleep(1500);
    const s = await ev(SNAP);
    if (s === last) stable++; else { stable = 0; last = s; }
    if (stable >= 4 && Date.now() - t0 > 5000) break;
  }
  console.log(`elapsed ${(Date.now() - t0) / 1000}s`);
  console.log(JSON.stringify(JSON.parse(last), null, 1));
} else if (cmd === 'approve') {
  await ev(`document.querySelector('#btn-approve-plan')?.click()`);
  const t0 = Date.now(); let last = '', stable = 0;
  while (Date.now() - t0 < (Number(arg) || 300) * 1000) {
    await sleep(2000);
    const s = await ev(SNAP);
    if (s === last) stable++; else { stable = 0; last = s; }
    if (stable >= 6) break;
  }
  console.log(`elapsed ${(Date.now() - t0) / 1000}s`);
  console.log(JSON.stringify(JSON.parse(last), null, 1));
} else if (cmd === 'snap') {
  console.log(JSON.stringify(JSON.parse(await ev(SNAP)), null, 1));
} else if (cmd === 'eval') {
  console.log(JSON.stringify(await ev(arg), null, 1));
} else if (cmd === 'page') {
  console.log(await ev(`document.querySelector('#nav-${arg}')?.click(), 'ok'`));
} else if (cmd === 'shot') {
  const m = await send('Page.captureScreenshot', { format: 'png' });
  writeFileSync(arg, Buffer.from(m.result.data, 'base64')); console.log('saved', arg);
}
process.exit(0);
