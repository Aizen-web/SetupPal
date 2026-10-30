// Fails when the app would load anything from a remote host at runtime: fonts, scripts, or styles.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const failures = [];

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (/\.(js|css|html)$/.test(name)) out.push(full);
  }
  return out;
}

const remoteLoad = /(?:@import\s+(?:url\()?['"]?https?:|src\s*=\s*['"]https?:|url\(\s*['"]?https?:|<link[^>]+href\s*=\s*['"]https?:|fonts\.(?:googleapis|gstatic)\.com)/i;
for (const file of [join(root, 'index.html'), ...walk(join(root, 'src'))]) {
  readFileSync(file, 'utf8').split(/\r?\n/).forEach((line, i) => {
    if (remoteLoad.test(line)) failures.push(`${file}:${i + 1}: remote load: ${line.trim()}`);
  });
}

const conf = JSON.parse(readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
for (const key of ['csp', 'devCsp']) {
  const csp = conf.app?.security?.[key] ?? '';
  for (const directive of ['style-src', 'font-src', 'script-src', 'default-src']) {
    const part = csp.split(';').map(s => s.trim()).find(s => s.startsWith(`${directive} `)) ?? '';
    if (/https?:\/\//.test(part)) failures.push(`${key} ${directive} allows a remote host: ${part}`);
  }
}

if (failures.length) {
  console.error(failures.join('\n'));
  process.exit(1);
}
console.log('Offline checks passed (no remote fonts, scripts, or styles; CSP is local only).');
