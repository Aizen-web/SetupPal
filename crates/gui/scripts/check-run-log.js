#!/usr/bin/env node
/**
 * Checks the run-log helpers: dry-run labels and the once-per-conversation scan line.
 * Run: node ./scripts/check-run-log.js
 */
import { labelRunText, runSummaryText, scanLogEntry, scanSignature } from '../src/api/run-log.js';

const failures = [];
const expect = (name, ok, detail = '') => { if (!ok) failures.push(`${name} ${detail}`.trim()); };

const profile = (tools) => ({
  installed_tools: tools.map(([binary, version]) => ({ binary, version_output: version })),
});

// Dry-run labels
expect('dry label prefixes', labelRunText('x', true) === '[DRY RUN] x');
expect('live text untouched', labelRunText('x', false) === 'x');
expect('dry summary names dry run', /^DRY RUN/.test(runSummaryText({ dryRun: true, passed: true })));
expect('dry summary says nothing changed', /nothing was installed/i.test(runSummaryText({ dryRun: true, passed: true })));
expect('dry failure still labelled', /^DRY RUN/.test(runSummaryText({ dryRun: true, passed: false })));
expect('live summary has no dry run', !/dry run/i.test(runSummaryText({ dryRun: false, passed: true })));
expect('backend headline wins', runSummaryText({ dryRun: false, passed: true, headline: 'Run finished: 2 steps.' }) === 'Run finished: 2 steps.');
expect('dry headline from old backend gets labelled', runSummaryText({ dryRun: true, passed: true, headline: 'Done.' }).startsWith('[DRY RUN]'));

// Scan line dedupe
const a = profile([['node', 'v22'], ['git', 'git 2.45']]);
const first = scanLogEntry([], a);
expect('first scan logs', first && first.text === '~ detected 2 installed tools', JSON.stringify(first));
expect('repeat scan is silent', scanLogEntry([first], a) === null);
expect('repeat after other logs is silent', scanLogEntry([first, { text: '> step', type: 'info' }], a) === null);
expect('order does not matter', scanLogEntry([first], profile([['git', 'git 2.45'], ['node', 'v22']])) === null);
const changed = scanLogEntry([first], profile([['node', 'v22'], ['git', 'git 2.45'], ['python', '3.12']]));
expect('changed facts log again', changed && changed.text === '~ detected 3 installed tools', JSON.stringify(changed));
expect('a version change logs again', scanLogEntry([first], profile([['node', 'v24'], ['git', 'git 2.45']])) !== null);
expect('new conversation (empty logs) logs again', scanLogEntry([], a) !== null);
expect('no tools means no line', scanLogEntry([], profile([])) === null);
expect('singular wording', scanLogEntry([], profile([['node', 'v22']])).text === '~ detected 1 installed tool');
expect('signature stable', scanSignature(a) === scanSignature(profile([['git', 'git 2.45'], ['node', 'v22']])));

if (failures.length) {
  console.error(`Run log check failed (${failures.length}):`);
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}
console.log('Run log check passed: dry runs are labelled, scan line shows once per set of facts.');
