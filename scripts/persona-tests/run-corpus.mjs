#!/usr/bin/env node
/**
 * SetupPal persona corpus runner.
 *
 *   node scripts/persona-tests/run-corpus.mjs --deterministic
 *   node scripts/persona-tests/run-corpus.mjs --live [--provider nebius] [--runs 1]
 *
 * Deterministic mode is key-free: schema + fixture ID checks only.
 * Live mode drives a running SetupPal window via CDP (port 9333) and never clicks Approve.
 */
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  assertAlreadyInstalled,
  assertClarify,
  assertConfirm,
  assertManualCard,
  assertPlanSteps,
  connectCdp,
  extractPlanIds,
} from './drive.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const CORPUS_PATH = join(__dirname, 'corpus.json');

/** Known winget / registry IDs used by the release corpus (fixtures for deterministic checks). */
const KNOWN_IDS = new Set([
  'VideoLAN.VLC',
  'Audacity.Audacity',
  'LMMS.LMMS',
  'Roblox.Roblox',
  'Spotify.Spotify',
  '7zip.7zip',
  'Notepad++.Notepad++',
  'Discord.Discord',
  'Valve.Steam',
  'EpicGames.EpicGamesLauncher',
  'OBSProject.OBSStudio',
  'Guru3D.Afterburner',
  'REALiX.HWiNFO',
  'Python.Python.3.12',
  'Python.Python.3.13',
  'OpenJS.NodeJS.LTS',
  'Git.Git',
  'Microsoft.VisualStudioCode',
  'BurntSushi.ripgrep.MSVC',
  'jqlang.jq',
  'Rustlang.Rust.MSVC',
  'Kitware.CMake',
  'Ninja-build.Ninja',
  'Oracle.JDK.21',
  'Adoptium.Temurin.21.JDK',
  'Mojang.MinecraftLauncher',
  'typescript',
  'requests',
  'lodash',
  'colorama',
  'numpy',
  'pandas',
  'HandBrake.HandBrake',
  'Microsoft.PowerToys',
  'Rustlang.Rustup',
]);

const CONFIRM_MAP = {
  reqeusts: 'requests',
  reqwests: 'requests',
  lodahs: 'lodash',
  colourama: 'colorama',
  typescipt: 'typescript',
};

function parseArgs(argv) {
  const out = { mode: null, provider: null, runs: 1, port: 9333, filter: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--deterministic') out.mode = 'deterministic';
    else if (a === '--live') out.mode = 'live';
    else if (a === '--provider') out.provider = argv[++i];
    else if (a === '--runs') out.runs = Math.max(1, Number(argv[++i]) || 1);
    else if (a === '--port') out.port = Number(argv[++i]) || 9333;
    else if (a === '--filter') out.filter = argv[++i];
    else if (a === '--help' || a === '-h') out.help = true;
  }
  return out;
}

function loadCorpus() {
  const raw = JSON.parse(readFileSync(CORPUS_PATH, 'utf8'));
  if (!raw || !Array.isArray(raw.cases)) {
    throw new Error('corpus.json must contain a cases array');
  }
  return raw;
}

function validateCaseSchema(c, idx) {
  const errors = [];
  if (!c.id || typeof c.id !== 'string') errors.push(`cases[${idx}]: missing id`);
  if (!c.persona || !['gamer', 'developer', 'casual'].includes(c.persona)) {
    errors.push(`${c.id || idx}: persona must be gamer|developer|casual`);
  }
  if (!c.request || typeof c.request !== 'string') errors.push(`${c.id || idx}: missing request`);
  const e = c.expect;
  if (!e || typeof e !== 'object') {
    errors.push(`${c.id || idx}: missing expect`);
    return errors;
  }
  const kinds = new Set(['install_ids', 'manual', 'clarify', 'already_installed', 'confirm']);
  if (!kinds.has(e.kind)) errors.push(`${c.id || idx}: bad expect.kind ${e.kind}`);
  if (e.kind === 'install_ids') {
    const ids = Array.isArray(e.ids) ? e.ids : [];
    const anyOf = Array.isArray(e.ids_any_of) ? e.ids_any_of : [];
    if (ids.length === 0 && anyOf.length === 0 && !e.recipe) {
      errors.push(`${c.id || idx}: install_ids needs ids, ids_any_of, or recipe`);
    }
    for (const id of [...ids, ...anyOf]) {
      if (
        !KNOWN_IDS.has(id) &&
        !/^[A-Za-z0-9][A-Za-z0-9+_.-]*\.[A-Za-z0-9]/.test(id) &&
        !/^[A-Za-z0-9][A-Za-z0-9+_.-]*$/.test(id)
      ) {
        errors.push(`${c.id || idx}: unrecognized id shape ${id}`);
      }
    }
  }
  if (e.kind === 'confirm') {
    if (!e.typed || !e.suggestion) errors.push(`${c.id || idx}: confirm needs typed + suggestion`);
  }
  if (e.kind === 'manual' && e.item == null && !(e.contains || e.item_contains || []).length) {
    // item / contains / item_contains optional but preferred
  }
  return errors;
}

/** Deterministic fixture checks that do not need the app or network. */
function deterministicAssertions(cases) {
  const failures = [];
  for (const c of cases) {
    const e = c.expect;
    if (e.kind === 'install_ids') {
      const ids = Array.isArray(e.ids) ? e.ids : [];
      const anyOf = Array.isArray(e.ids_any_of) ? e.ids_any_of : [];
      for (const id of ids) {
        if (!KNOWN_IDS.has(id)) {
          // Still allow dotted winget IDs not in the fixture set, but flag bare unknowns.
          if (!id.includes('.') && !['typescript', 'requests', 'lodash', 'colorama'].includes(id)) {
            failures.push(`${c.id}: install id not in fixture set: ${id}`);
          }
        }
      }
      if (e.max_install_steps != null && ids.length > e.max_install_steps) {
        failures.push(`${c.id}: ids.length > max_install_steps`);
      }
      // Single-app requests must not list a recipe bundle.
      const lower = c.request.toLowerCase().trim();
      if (/^install (vlc|audacity|lmms|discord|spotify|roblox|steam)(?:\s+please)?$/.test(lower)) {
        if (ids.length !== 1) failures.push(`${c.id}: single-app request must expect exactly one id`);
      }
      if (anyOf.length === 0 && ids.length === 0 && !e.recipe) {
        failures.push(`${c.id}: install_ids case has no expected packages`);
      }
    }
    if (e.kind === 'confirm') {
      const mapped = CONFIRM_MAP[e.typed];
      if (mapped && mapped !== e.suggestion) {
        failures.push(`${c.id}: expected suggestion ${mapped} for ${e.typed}`);
      }
      if (e.typed === e.suggestion) {
        failures.push(`${c.id}: confirm typed must differ from suggestion`);
      }
    }
    if (e.kind === 'manual') {
      const lower = c.request.toLowerCase();
      if (!/bedrock|driver|store|minecraft bedrock/.test(lower) && !(e.contains || []).length) {
        // Broad manual without a Store/driver cue is suspicious for this corpus.
        if (/wanna game|make music|i'?m a student|learning to code/.test(lower)) {
          failures.push(`${c.id}: broad request should be clarify, not manual`);
        }
      }
    }
    if (e.kind === 'clarify') {
      const lower = c.request.toLowerCase();
      if (/^install [a-z0-9.+-]+$/.test(lower.trim())) {
        failures.push(`${c.id}: exact install request should not be clarify`);
      }
    }
  }
  return failures;
}

async function runDeterministic() {
  const corpus = loadCorpus();
  const cases = corpus.cases;
  console.log(`Corpus v${corpus.version || '?'} — ${cases.length} cases (deterministic)`);

  const schemaErrors = [];
  const ids = new Set();
  cases.forEach((c, i) => {
    if (ids.has(c.id)) schemaErrors.push(`duplicate id ${c.id}`);
    ids.add(c.id);
    schemaErrors.push(...validateCaseSchema(c, i));
  });
  if (cases.length < 60) {
    schemaErrors.push(`need at least 60 cases, found ${cases.length}`);
  }

  const fixtureErrors = deterministicAssertions(cases);
  const all = [...schemaErrors, ...fixtureErrors];
  if (all.length) {
    console.error('FAILED deterministic corpus:');
    for (const err of all) console.error(`  - ${err}`);
    process.exit(1);
  }
  console.log('Deterministic corpus OK.');
  process.exit(0);
}

function evaluateLiveSnap(c, snap) {
  const e = c.expect;
  switch (e.kind) {
    case 'install_ids': {
      const ids = Array.isArray(e.ids) ? e.ids : [];
      const anyOf = Array.isArray(e.ids_any_of) ? e.ids_any_of : [];
      if (ids.length) {
        const result = assertPlanSteps(snap, ids, { allowExtra: false });
        if (!result.ok) {
          return {
            ok: false,
            detail: `missing=${result.missing.join(',')} unexpected=${result.unexpected.join(',')} found=${result.found.join(',')}`,
          };
        }
      } else if (anyOf.length) {
        const found = extractPlanIds(snap);
        const hit = anyOf.some((want) => found.some((f) => f === want || f.includes(want) || want.includes(f)));
        if (!hit && !e.recipe) {
          return { ok: false, detail: `none of ids_any_of matched; found=${found.join(',')}` };
        }
        // Recipe plans may expose recipe name / summary rather than winget IDs.
        if (!hit && e.recipe) {
          const blob = `${snap.planTitle || ''}\n${snap.planCard || ''}`.toLowerCase();
          if (!blob.includes(String(e.recipe).toLowerCase()) && !/(fabric|minecraft)/.test(blob)) {
            return { ok: false, detail: `recipe ${e.recipe} not evident in plan card` };
          }
        }
      } else if (e.recipe) {
        const blob = `${snap.planTitle || ''}\n${snap.planCard || ''}`.toLowerCase();
        if (!blob.includes(String(e.recipe).toLowerCase()) && !snap.approveBtn) {
          return { ok: false, detail: `expected recipe plan for ${e.recipe}` };
        }
      }
      if (e.max_install_steps != null) {
        const installs = extractPlanIds(snap);
        if (installs.length > e.max_install_steps) {
          return { ok: false, detail: `too many install ids: ${installs.join(',')}` };
        }
      }
      return { ok: true };
    }
    case 'manual': {
      const needles = e.contains || e.item_contains || (e.item ? [e.item] : []);
      return assertManualCard(snap, needles);
    }
    case 'clarify':
      return assertClarify(snap);
    case 'already_installed':
      return assertAlreadyInstalled(snap, e.ids || []);
    case 'confirm':
      return assertConfirm(snap, { typed: e.typed, suggestion: e.suggestion });
    default:
      return { ok: false, detail: `unknown kind ${e.kind}` };
  }
}

async function runLive({ provider, runs, port, filter }) {
  let session;
  try {
    session = await connectCdp({ port });
  } catch (err) {
    console.error(err.message);
    console.error('Live corpus requires SetupPal running with remote debugging, for example:');
    console.error(`  $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=${port}'`);
    console.error('  npm --prefix crates/gui run app');
    process.exit(2);
  }

  const corpus = loadCorpus();
  let cases = corpus.cases;
  if (filter) cases = cases.filter((c) => c.id.includes(filter) || c.persona === filter);
  console.log(`Live corpus — ${cases.length} cases × ${runs} run(s)${provider ? ` provider=${provider}` : ''}`);

  await session.enableMonitoring();
  const failures = [];
  try {
    for (let run = 1; run <= runs; run++) {
      for (const c of cases) {
        process.stdout.write(`[${run}/${runs}] ${c.id} … `);
        await session.newChat();
        const { snap } = await session.say(c.request);
        const result = evaluateLiveSnap(c, snap);
        const pageProblems = await session.pageEvents();
        const consoleProblems = session.consoleProblems();
        const noise = [...pageProblems, ...consoleProblems];
        if (!result.ok || noise.length) {
          const detail = result.ok ? '' : (result.detail || result.reason || 'assert failed');
          const noiseMsg = noise.length ? ` console=${JSON.stringify(noise)}` : '';
          console.log('FAIL');
          failures.push(`${c.id}: ${detail}${noiseMsg}`);
        } else {
          console.log('ok');
        }
      }
    }
  } finally {
    session.close();
  }

  if (failures.length) {
    console.error(`\n${failures.length} failure(s):`);
    for (const f of failures) console.error(`  - ${f}`);
    process.exit(1);
  }
  console.log('Live corpus OK.');
  process.exit(0);
}

const args = parseArgs(process.argv.slice(2));
if (args.help || !args.mode) {
  console.log(`Usage:
  node ${resolve('scripts/persona-tests/run-corpus.mjs')} --deterministic
  node ${resolve('scripts/persona-tests/run-corpus.mjs')} --live [--provider nebius] [--runs N] [--filter idOrPersona]
`);
  process.exit(args.help ? 0 : 1);
}

if (args.mode === 'deterministic') await runDeterministic();
else await runLive(args);
