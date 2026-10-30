#!/usr/bin/env node
/**
 * Checks that the chat formatter never turns model text into live HTML or unsafe links.
 * Run: node ./scripts/check-formatter.js
 */
import { formatChatMessage } from '../src/components/chat-formatter.js';
import { esc, safeHttpsUrl } from '../src/components/html.js';

const failures = [];
const render = (text) => formatChatMessage(text, { msgId: 'test', interactive: true });

function expectAbsent(name, input, pattern) {
  const out = render(input);
  if (pattern.test(out)) failures.push(`${name}: found ${pattern} in\n    ${out.slice(0, 400)}`);
}

function expectPresent(name, input, pattern) {
  const out = render(input);
  if (!pattern.test(out)) failures.push(`${name}: expected ${pattern} in\n    ${out.slice(0, 400)}`);
}

expectAbsent('raw img tag', '<img src=x onerror=alert(1)>', /<img/i);
expectAbsent('raw script tag', 'hi <script>alert(1)</script>', /<script/i);
expectAbsent('javascript link', '[click me](javascript:alert(1))', /href="javascript:/i);
expectAbsent('file link', '[open](file:///C:/Windows/System32)', /href="file:/i);
expectAbsent('data link', '[x](data:text/html,<b>x</b>)', /href="data:/i);
expectAbsent('attribute break in link label', '[a" onmouseover="x](https://example.com)', /" onmouseover="/i);
expectAbsent('html in code block', '```html\n<iframe src="https://evil.test"></iframe>\n```', /<iframe/i);
expectAbsent('html in table', '| a | b |\n|---|---|\n| <b onclick=x>1</b> | 2 |', /<b onclick/i);
expectAbsent('html in heading', '## <svg onload=alert(1)>', /<svg onload/i);
expectAbsent('html in list', '- <a href="javascript:x">x</a>', /<a href="javascript/i);
expectPresent('https link still works', 'See [the docs](https://nodejs.org/en/download).', /href="https:\/\/nodejs\.org\/en\/download"/);

if (esc(`<"'&>`) !== '&lt;&quot;&#39;&amp;&gt;') failures.push(`esc(): got ${esc(`<"'&>`)}`);
if (safeHttpsUrl('javascript:alert(1)')) failures.push('safeHttpsUrl allowed javascript:');
if (safeHttpsUrl('http://example.com')) failures.push('safeHttpsUrl allowed plain http:');
if (!safeHttpsUrl('https://example.com/x')) failures.push('safeHttpsUrl rejected an https URL');

if (failures.length) {
  console.error(`Formatter check failed (${failures.length}):`);
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}
console.log('Formatter check passed: no raw HTML, only https links.');
