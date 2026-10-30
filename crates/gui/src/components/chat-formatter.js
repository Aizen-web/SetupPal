/**
 * SetupPal: Chat message formatter
 *
 * Turns an assistant reply (markdown) into compact, readable UI.
 *
 * Message anatomy (every part is optional):
 *   1. Lead sentence        what I understood + "nothing has changed yet"
 *   2. System check         collapsible list of only the facts that matter, one row per fact
 *   3. Your choices         radio groups + one Continue button (answers sent together)
 *   4. Sources              domains that will be checked
 *   5. Next note           one quiet line saying what happens next
 *
 * Plus standard markdown: headings, lists, task lists, tables, quotes, code blocks, alerts.
 *
 * Design rules (see ui.md 6.1, 16):
 *   - one signal per status: icon + word, colour only reinforces
 *   - no decorative emoji, no internal labels ("Section 1", "Live scan")
 *   - things that do not apply are muted, never red
 *   - only real failures use the error colour
 */

import { esc } from './html.js';

/* ------------------------------------------------------------------ */
/* Helpers                                                             */
/* ------------------------------------------------------------------ */

export const escapeHtml = esc;

const ICON_PATHS = {
  check: '<polyline points="20 6 9 17 4 12"/>',
  alert: '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>',
  missing: '<circle cx="12" cy="12" r="9"/><path d="M12 8v5"/><path d="M12 16h.01"/>',
  error: '<circle cx="12" cy="12" r="9"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>',
  skip: '<circle cx="12" cy="12" r="9"/><path d="M8 12h8"/>',
  info: '<circle cx="12" cy="12" r="2.5" fill="currentColor"/>',
  arrow: '<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>',
  chevron: '<path d="m6 9 6 6 6-6"/>',
  copy: '<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>',
  star: '<polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" fill="currentColor"/>',
  shield: '<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/>',
  globe: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3a14 14 0 0 1 0 18 14 14 0 0 1 0-18"/>',
};

export function icon(name, size = 14) {
  return `<svg class="ico ico-${name}" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${ICON_PATHS[name] || ''}</svg>`;
}

/** Strips markdown emphasis / code ticks to plain text. */
function plain(text) {
  return String(text || '')
    .replace(/[`*_]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

function looksLikePath(s) {
  return /^(?:[A-Za-z]:[\\/]|\\\\|%[A-Za-z_]+%|~[\\/]|\.{1,2}[\\/]|\/)/.test(s);
}

/** Keeps the tail of long paths ("…\Roaming\.minecraft"). Full path stays in title + copy. */
function shortenPath(p, max = 40) {
  if (p.length <= max) return p;
  const sep = p.includes('\\') ? '\\' : '/';
  const parts = p.split(/[\\/]/).filter(Boolean);
  let tail = '';
  for (let k = parts.length - 1; k >= 0; k--) {
    const cand = tail ? `${parts[k]}${sep}${tail}` : parts[k];
    if (cand.length + 2 > max) break;
    tail = cand;
  }
  return tail ? `…${sep}${tail}` : `…${p.slice(-(max - 1))}`;
}

/* ------------------------------------------------------------------ */
/* Inline markdown                                                     */
/* ------------------------------------------------------------------ */

/**
 * Inline markdown: code, bold, italic, strike, links.
 * Code spans and links are stashed first so their contents are never re-parsed
 * (fixes mangled snake_case paths and bold-inside-code).
 */
export function formatInline(text) {
  if (!text) return '';

  const stash = [];
  const hold = (html) => `\u0000${stash.push(html) - 1}\u0001`;

  let out = escapeHtml(text);

  // 1. Code spans
  out = out.replace(/`([^`]+)`/g, (_, code) => hold(`<code class="chat-code-pill">${code}</code>`));

  // 2. Links (before emphasis so URL underscores survive). Only http(s) targets become links.
  out = out.replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g, (_, label, url) =>
    hold(`<a href="${url}" target="_blank" rel="noopener noreferrer" class="chat-inline-link">${label}<span class="ext-arrow">${icon('arrow', 12)}</span></a>`));

  // 3. Emphasis
  out = out.replace(/\*\*(?!\s)([^\n]*?[^\s])\*\*/g, '<strong class="chat-bold">$1</strong>');
  out = out.replace(/(^|[^\w])__(?!\s)([^_\n]*[^_\s])__(?!\w)/g, '$1<strong class="chat-bold">$2</strong>');
  out = out.replace(/~~(?!\s)([^~\n]*[^~\s])~~/g, '<del class="chat-del">$1</del>');
  out = out.replace(/(^|[^*\w])\*(?!\s)([^*\n]*[^*\s])\*(?![*\w])/g, '$1<em class="chat-italic">$2</em>');
  out = out.replace(/(^|[^\w])_(?!\s)([^_\n]*[^_\s])_(?!\w)/g, '$1<em class="chat-italic">$2</em>');

  // 4. Restore stashed fragments (links may contain code spans, so repeat)
  for (let pass = 0; pass < 4 && out.includes('\u0000'); pass++) {
    out = out.replace(/\u0000(\d+)\u0001/g, (_, n) => stash[Number(n)]);
  }
  return out;
}

/* ------------------------------------------------------------------ */
/* Status model                                                        */
/* ------------------------------------------------------------------ */

const STATUS_DEFAULT_WORD = {
  ok: 'OK',
  warn: 'Attention',
  missing: 'Missing',
  error: 'Failed',
  skip: 'Not needed',
  info: '',
};

const STATUS_ICON = {
  ok: 'check',
  warn: 'alert',
  missing: 'missing',
  error: 'error',
  skip: 'skip',
  info: 'info',
};

/** Order matters: "not installed" must match `missing` before `installed` matches `ok`. */
function mapStatus(text) {
  const t = text.toLowerCase();
  if (/skip|not needed|n\/a|not applicable|optional|unused|not used/.test(t)) return 'skip';
  if (/\b(fail(ed|ure)?|error|blocked?|denied|mismatch|unsafe)\b/.test(t)) return 'error';
  if (/miss|absent|not found|not detected|not installed|\bnone\b/.test(t)) return 'missing';
  if (/attention|caution|warn|held|outdated|too old|upgrade/.test(t)) return 'warn';
  if (/verified|installed|\bok\b|ready|found|detected|good|pass|present|available|safe/.test(t)) return 'ok';
  return 'info';
}

/* ------------------------------------------------------------------ */
/* System check                                                        */
/* ------------------------------------------------------------------ */

const KV_RE = /^\s*[•\-*]\s*\*\*([^*]+?)\*\*\s*:?\s*(.*)$/;

function parseKeyValueItems(lines) {
  const items = [];
  for (const line of lines) {
    if (!line.trim()) continue;
    const m = line.match(KV_RE);
    if (!m) {
      // indented continuation of the previous value
      if (items.length && /^\s{2,}\S/.test(line)) {
        items[items.length - 1].raw += ' ' + line.trim();
      }
      continue;
    }
    items.push({ label: m[1].replace(/:\s*$/, '').trim(), raw: m[2].trim() });
  }

  return items.map(({ label, raw }) => {
    let value = raw;
    let status = 'info';
    let word = '';

    const sm = value.match(/\((?:Status:\s*)([^)]+)\)/i);
    if (sm) {
      value = value.replace(sm[0], '').replace(/\s{2,}/g, ' ').trim();
      const explicit = sm[1].trim();
      status = mapStatus(explicit);
      word = explicit.length <= 20 ? explicit : STATUS_DEFAULT_WORD[status];
    } else {
      const lower = value.toLowerCase();
      if (value.includes('❌') || /not detected|absent|missing|not installed|not found/.test(lower)) {
        status = 'missing';
      } else if (value.includes('⚠') || /caution|attention/.test(lower)) {
        status = 'warn';
      } else if (value.includes('✅') || /\b(installed|verified)\b/.test(lower)) {
        status = 'ok';
      }
      word = STATUS_DEFAULT_WORD[status];
    }

    value = value.replace(/^[✅❌⚠️🛡️⚡\uFE0F\s]+/u, '').replace(/^Detected at\s*/i, '').trim();

    // " — note" tail
    let note = '';
    const parts = value.split(/\s+[—–]\s+/);
    if (parts.length > 1) {
      value = parts[0].trim();
      note = parts.slice(1).join(' — ').trim();
    }

    return { label, value, note, status, word };
  });
}

function renderSysValue(value) {
  const only = value.match(/^`([^`]+)`$/);
  if (only && looksLikePath(only[1])) {
    const full = only[1];
    const short = shortenPath(full);
    return `
      <span class="sys-path-wrap">
        <code class="chat-code-pill sys-path" title="${escapeHtml(full)}">${escapeHtml(short)}</code>
        <button type="button" class="sys-copy" data-copy="${escapeHtml(full)}" title="Copy path" aria-label="Copy path">${icon('copy', 12)}</button>
      </span>`;
  }
  return formatInline(value);
}

function cleanSectionTitle(title) {
  return title
    .replace(/^Section\s+\d+\s*[:.-]?\s*/i, '')
    .replace(/\s*\((?:Key-Value Grid|Grid)\)\s*$/i, '')
    .replace(/[`*_]/g, '')
    .trim();
}

function renderSystemCheck(rawTitle, items, ctx) {
  const idx = ctx.counters.sys++;
  const key = `${ctx.msgId}:sys${idx}`;

  let title = cleanSectionTitle(rawTitle || '');
  if (!title || /host|pre-?flight|environment|diagnos/i.test(title)) title = 'System check';

  const shown = items.filter(i => i.status !== 'skip').concat(items.filter(i => i.status === 'skip'));
  const attention = items.filter(i => ['warn', 'missing', 'error'].includes(i.status)).length;
  const hasOk = items.some(i => i.status === 'ok');

  let meta = '';
  let metaClass = '';
  if (attention > 0) {
    meta = `${attention} ${attention === 1 ? 'needs' : 'need'} attention`;
    metaClass = 'warn';
  } else if (hasOk) {
    meta = 'All clear';
    metaClass = 'ok';
  }

  const isOpen = !ctx.collapsed.has(key);

  return `
    <details class="chat-sys" data-key="${escapeHtml(key)}" ${isOpen ? 'open' : ''}>
      <summary>
        <span class="sys-title">${escapeHtml(title)}</span>
        ${meta ? `<span class="sys-meta ${metaClass}">${escapeHtml(meta)}</span>` : ''}
        <span class="sys-chev">${icon('chevron', 14)}</span>
      </summary>
      <div class="sys-rows">
        ${shown.map(it => `
          <div class="sys-row ${it.status}">
            <span class="sys-icon">${icon(STATUS_ICON[it.status], 14)}</span>
            <span class="sys-label">${escapeHtml(it.label)}</span>
            <span class="sys-value">
              ${renderSysValue(it.value)}
              ${it.note ? `<span class="sys-note">${formatInline(it.note)}</span>` : ''}
            </span>
            <span class="sys-status">${escapeHtml(it.word)}</span>
          </div>
        `).join('')}
      </div>
    </details>`;
}

/* ------------------------------------------------------------------ */
/* Choices                                                             */
/* ------------------------------------------------------------------ */

const NUM_ITEM_RE = /^\s*(\d+)[.)]\s+(?:\*\*([^*]+?)\*\*\s*:?|([^:*`|]{2,40}?):)\s*(.*)$/;

function splitOptions(rest) {
  if (rest.includes('|')) return rest.split(/\s*\|\s*/).filter(Boolean);
  if (/\s+vs\.?\s+/i.test(rest)) return rest.split(/\s+vs\.?\s+/i).filter(Boolean);

  const spans = rest.match(/`[^`]+`(?:\s*\([^)]*\))?/g) || [];
  if (spans.length >= 2) {
    const leftover = rest
      .replace(/`[^`]+`(?:\s*\([^)]*\))?/g, '')
      .replace(/\b(e\.g\.|or|and|such as)\b/gi, '')
      .replace(/[\s,;:/.\-*_()]/g, '');
    if (leftover.length <= 6) return spans;
  }
  return [];
}

function parseOption(raw) {
  const s = raw.trim().replace(/^[\s,;]+|[\s,;]+$/g, '');

  const cm = s.match(/`([^`]+)`/);
  const headEnd = cm ? cm.index + cm[0].length : 0;
  const tail = s.slice(headEnd);
  const di = tail.search(/\s[—–]\s|\s-\s/);

  let desc = '';
  let main = s;
  if (di >= 0) {
    desc = tail.slice(di).replace(/^\s*[—–-]\s*/, '').trim();
    main = s.slice(0, headEnd + di);
  }

  let label;
  let rest;
  if (cm) {
    label = cm[1];
    rest = main.slice(0, cm.index) + ' ' + main.slice(headEnd);
  } else {
    const pm = main.match(/^(.*?)\s*(\(.*)$/);
    label = pm ? pm[1] : main;
    rest = pm ? pm[2] : '';
  }
  label = plain(label);

  let recommended = false;
  let tag = '';
  rest.replace(/\(([^)]*)\)/g, (_, t) => {
    const text = t.trim();
    if (/recommended/i.test(text)) {
      recommended = true;
      const extra = text.replace(/^recommended\s*(for)?\s*/i, '').trim();
      if (extra && !desc) desc = `Recommended for ${extra}`;
    } else if (!tag) {
      tag = text;
    }
    return '';
  });

  return { label, tag: plain(tag), desc: plain(desc), recommended };
}

function parseChoiceGroups(lines) {
  const raw = [];
  for (const line of lines) {
    if (!line.trim()) continue;
    const m = line.match(NUM_ITEM_RE);
    if (m) {
      raw.push({
        num: m[1],
        name: plain(m[2] || m[3]).replace(/:\s*$/, ''),
        rest: (m[4] || '').trim(),
      });
    } else if (raw.length && /^\s{2,}\S/.test(line)) {
      raw[raw.length - 1].rest += ' ' + line.trim();
    }
  }

  return raw.map(g => {
    const options = splitOptions(g.rest).map(parseOption).filter(o => o.label);
    return { num: g.num, name: g.name, rest: g.rest, options: options.length >= 2 ? options : [] };
  });
}

function renderChoiceForm(groups, ctx, nextText) {
  const formId = `${ctx.msgId}-f${ctx.counters.form++}`;
  const live = ctx.interactive;
  const picked = [];

  const groupsHtml = groups.map((g, gi) => {
    const head = `
      <div class="choice-head" id="${escapeHtml(formId)}-h${gi}">
        <span class="choice-num">${escapeHtml(g.num)}</span>
        <span class="choice-name">${escapeHtml(g.name)}</span>
      </div>`;

    if (g.options.length === 0) {
      return `
        <div class="choice-group">
          ${head}
          ${g.rest ? `<div class="choice-text">${formatInline(g.rest)}</div>` : ''}
        </div>`;
    }

    const recIdx = g.options.findIndex(o => o.recommended);
    const saved = ctx.selections[`${formId}:${gi}`];
    const selIdx = Number.isInteger(saved) && saved < g.options.length ? saved : (recIdx >= 0 ? recIdx : 0);
    picked.push(g.options[selIdx].label);

    const layout = g.options.some(o => o.desc) ? 'cards' : 'chips';

    return `
      <div class="choice-group" role="radiogroup" aria-labelledby="${escapeHtml(formId)}-h${gi}" data-name="${escapeHtml(g.name)}">
        ${head}
        <div class="choice-options layout-${layout}">
          ${g.options.map((o, oi) => `
            <label class="choice-opt">
              <input type="radio" class="choice-input" name="${escapeHtml(formId)}-g${gi}" value="${escapeHtml(o.label)}"
                data-form="${escapeHtml(formId)}" data-gi="${gi}" data-oi="${oi}" ${oi === selIdx ? 'checked' : ''} ${live ? '' : 'disabled'}>
              <span class="choice-card">
                <span class="choice-radio" aria-hidden="true"></span>
                <span class="choice-main">
                  <span class="choice-line">
                    <span class="choice-label">${escapeHtml(o.label)}</span>
                    ${o.tag ? `<span class="choice-tag">${escapeHtml(o.tag)}</span>` : ''}
                    ${o.recommended ? `<span class="choice-rec">${icon('star', 10)}Recommended</span>` : ''}
                  </span>
                  ${o.desc ? `<span class="choice-desc">${escapeHtml(o.desc)}</span>` : ''}
                </span>
              </span>
            </label>
          `).join('')}
        </div>
      </div>`;
  }).join('');

  const footer = live && picked.length ? `
      <div class="choice-footer">
        <button type="button" class="chat-continue-btn" data-continue="${escapeHtml(formId)}">
          <span>Continue</span>${icon('arrow', 14)}
        </button>
        <span class="choice-summary" data-summary="${escapeHtml(formId)}">${escapeHtml(picked.join(' · '))}</span>
      </div>` : '';

  const note = live && nextText ? renderNextNote(nextText) : '';

  return `<div class="chat-choice-form${live ? '' : ' is-locked'}" data-form="${escapeHtml(formId)}">${groupsHtml}${footer}${note}</div>`;
}

/* ------------------------------------------------------------------ */
/* Small blocks                                                        */
/* ------------------------------------------------------------------ */

function renderNextNote(text) {
  return `<div class="chat-next-note"><span class="next-note-icon">${icon('arrow', 13)}</span><span>${formatInline(text)}</span></div>`;
}

function renderSources(listText) {
  const domains = listText
    .replace(/[`*]/g, '')
    .split(/\s*[·,|•]\s*/)
    .map(s => s.trim())
    .filter(Boolean);
  if (!domains.length) return '';
  return `
    <div class="chat-sources">
      <span class="sources-label">Sources I'll check</span>
      ${domains.map(d => `<span class="source-chip">${icon('globe', 12)}${escapeHtml(d)}</span>`).join('')}
    </div>`;
}

function renderAlert(kind, title, body) {
  return `
    <div class="chat-alert-banner alert-${kind}" role="${kind === 'warning' ? 'alert' : 'status'}">
      <span class="alert-icon-wrap">${icon(kind === 'warning' ? 'alert' : 'shield', 16)}</span>
      <div class="alert-body">
        <div class="alert-title">${formatInline(title)}</div>
        ${body ? `<div class="alert-desc">${formatInline(body)}</div>` : ''}
      </div>
    </div>`;
}

function renderTable(tableLines) {
  const parseRow = (line) => line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map(c => c.trim());
  const headers = parseRow(tableLines[0]);
  const rows = tableLines.slice(2).map(parseRow);
  return `
    <div class="chat-table-wrap">
      <table class="chat-table">
        <thead><tr>${headers.map(h => `<th>${formatInline(h)}</th>`).join('')}</tr></thead>
        <tbody>${rows.map(r => `<tr>${r.map(c => `<td>${formatInline(c)}</td>`).join('')}</tr>`).join('')}</tbody>
      </table>
    </div>`;
}

const SHELL_LANGS = new Set(['sh', 'bash', 'zsh', 'shell', 'console', 'powershell', 'ps1', 'cmd', 'bat', 'terminal']);

function renderCodeBlock(lang, code) {
  const label = !lang || lang === 'text' ? 'Text' : SHELL_LANGS.has(lang) ? 'Terminal' : lang.toUpperCase();
  return `
    <div class="chat-code-block-wrap">
      <div class="chat-code-header">
        <span class="chat-code-lang">${escapeHtml(label)}</span>
        <button type="button" class="chat-code-copy-btn" data-code="${escapeHtml(code)}" title="Copy code">
          ${icon('copy', 12)}<span>Copy</span>
        </button>
      </div>
      <pre class="chat-code-block"><code>${escapeHtml(code)}</code></pre>
    </div>`;
}

/* ------------------------------------------------------------------ */
/* Line parser                                                         */
/* ------------------------------------------------------------------ */

const BULLET_RE = /^\s*[•\-*]\s+/;
const NUM_RE = /^\s*\d+[.)]\s+/;
const TASK_RE = /^\s*[•\-*]\s+\[[ xX]\]\s*/;
const HEADING_RE = /^(#{1,4})\s+(.*)$/;
const CODE_RE = /^__CODE_BLOCK_(\d+)__$/;
const SOURCES_RE = /^[*_]*(?:Sources?(?:\s+(?:I'll|I will|to)\s+check)?|Will check)[*_]*\s*:\s*(.+)$/i;
const SEPARATOR_RE = /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$/;

function parseNext(trimmed) {
  const stripped = trimmed.replace(/^[`\s]+|[`\s]+$/g, '');
  const m = stripped.match(/^Next:\s*(.*)$/i);
  if (!m) return null;
  return m[1].replace(/\s*(→|->|&rarr;)\s*$/, '').trim();
}

function asHeading(trimmed) {
  const m = trimmed.match(HEADING_RE);
  if (m) return { level: m[1].length, title: m[2].trim() };
  // Legacy: bare "Host System Pre-Flight Diagnosis" line
  if (trimmed.length < 70 && /^[^\w]*(host system\s+)?pre-?flight diagnosis/i.test(trimmed.replace(/[*`]/g, ''))) {
    return { level: 3, title: trimmed.replace(/[^\w\s&-]/g, '').trim() };
  }
  return null;
}

function classifySection(title) {
  const t = title.toLowerCase();
  if (/scope|context|problem definition|overview/.test(t)) return 'summary';
  if (/choice|configuration|question|option|preference|parameter/.test(t)) return 'choices';
  if (/host|system|environment|diagnos|pre-?flight|check|your setup|setup|detected/.test(t)) return 'system';
  return null;
}

function listKind(line) {
  if (TASK_RE.test(line)) return 'task';
  if (NUM_RE.test(line)) return 'ol';
  if (BULLET_RE.test(line)) return 'ul';
  return null;
}

function isAlertStart(trimmed) {
  return trimmed.startsWith('⚠️') || trimmed.startsWith('⚠') || /^✅\s*\*\*/.test(trimmed);
}

function isBlockStart(lines, i) {
  const trimmed = lines[i].trim();
  if (!trimmed) return true;
  if (CODE_RE.test(trimmed) || asHeading(trimmed) || parseNext(trimmed) !== null) return true;
  if (SOURCES_RE.test(trimmed) || trimmed.startsWith('>') || isAlertStart(trimmed)) return true;
  if (listKind(lines[i])) return true;
  if (trimmed.includes('|') && i + 1 < lines.length && SEPARATOR_RE.test(lines[i + 1])) return true;
  return false;
}

/** Bullets squashed into one line: "Intro • A: x • B: y" */
function splitInlineBullets(text) {
  const hits = text.match(/(^|\s)•\s/g) || [];
  if (hits.length < 2) return null;
  const pieces = text.split(/\s*•\s+/);
  const lead = pieces.shift().trim();
  return { lead, items: pieces.map(p => p.trim()).filter(Boolean) };
}

function parseLines(lines, ctx) {
  const blocks = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];
    const trimmed = line.trim();

    if (!trimmed) { i++; continue; }

    // Fenced code
    const codeM = trimmed.match(CODE_RE);
    if (codeM) {
      const { lang, code } = ctx.codeBlocks[Number(codeM[1])];
      blocks.push({ kind: 'html', html: renderCodeBlock(lang, code) });
      i++;
      continue;
    }

    // Next line
    const nextText = parseNext(trimmed);
    if (nextText !== null) {
      blocks.push({ kind: 'next', text: nextText });
      i++;
      continue;
    }

    // Sources
    const srcM = trimmed.match(SOURCES_RE);
    if (srcM) {
      blocks.push({ kind: 'html', html: renderSources(srcM[1]) });
      i++;
      continue;
    }

    // Headings & sections
    const head = asHeading(trimmed);
    if (head) {
      i++;
      const type = classifySection(head.title);

      if (type === 'summary') {
        const body = [];
        while (i < lines.length && lines[i].trim()) body.push(lines[i++].trim());
        const text = body.join(' ').replace(/^[•\-*]\s*/, '');
        if (text) blocks.push({ kind: 'p', inline: formatInline(text) });
        continue;
      }

      if (type === 'system' || type === 'choices') {
        const body = [];
        while (i < lines.length) {
          const l = lines[i];
          if (!l.trim() || listKind(l) || /^\s{2,}\S/.test(l)) { body.push(l); i++; } else { break; }
        }

        if (type === 'system') {
          const items = parseKeyValueItems(body);
          if (items.length) {
            blocks.push({ kind: 'html', html: renderSystemCheck(head.title, items, ctx) });
            continue;
          }
        } else {
          const groups = parseChoiceGroups(body);
          if (groups.length) {
            blocks.push({ kind: 'choices', groups });
            continue;
          }
        }
        // Not parseable: show a plain heading and let the body parse normally.
        blocks.push({ kind: 'html', html: `<h3 class="chat-h3">${formatInline(cleanSectionTitle(head.title))}</h3>` });
        blocks.push(...parseLines(body, ctx));
        continue;
      }

      const lvl = Math.min(Math.max(head.level, 1), 4);
      blocks.push({ kind: 'html', html: `<h${lvl} class="chat-h${lvl}">${formatInline(cleanSectionTitle(head.title))}</h${lvl}>` });
      continue;
    }

    // Alerts
    if (isAlertStart(trimmed)) {
      const kind = trimmed.startsWith('✅') ? 'success' : 'warning';
      const title = trimmed.replace(/^(⚠️|⚠|✅)\s*/, '').replace(/^\*\*(.*?)\*\*:?/, '$1').trim();
      i++;
      const body = [];
      while (i < lines.length && lines[i].trim() && !isBlockStart(lines, i)) body.push(lines[i++].trim());
      blocks.push({ kind: 'html', html: renderAlert(kind, title, body.join(' ')) });
      continue;
    }

    // Table
    if (trimmed.includes('|') && i + 1 < lines.length && SEPARATOR_RE.test(lines[i + 1])) {
      const tbl = [];
      while (i < lines.length && lines[i].trim().includes('|')) tbl.push(lines[i++]);
      blocks.push({ kind: 'html', html: renderTable(tbl) });
      continue;
    }

    // Blockquote
    if (trimmed.startsWith('>')) {
      const q = [];
      while (i < lines.length && lines[i].trim().startsWith('>')) q.push(lines[i++].trim().replace(/^>\s?/, ''));
      blocks.push({
        kind: 'html',
        html: `<blockquote class="chat-blockquote"><div class="quote-content">${formatInline(q.join(' '))}</div></blockquote>`,
      });
      continue;
    }

    // Lists
    const kind = listKind(line);
    if (kind) {
      const items = [];
      while (i < lines.length) {
        const l = lines[i];
        if (!l.trim()) {
          // allow blank lines inside a list if the next non-blank line continues it
          let j = i + 1;
          while (j < lines.length && !lines[j].trim()) j++;
          if (j < lines.length && listKind(lines[j]) === kind) { i = j; continue; }
          break;
        }
        const k = listKind(l);
        if (k === kind) { items.push(l); i++; continue; }
        if (!k && /^\s{2,}\S/.test(l) && items.length) { items[items.length - 1] += ' ' + l.trim(); i++; continue; }
        break;
      }

      if (kind === 'ul' && items.every(l => KV_RE.test(l))) {
        const parsed = parseKeyValueItems(items);
        if (parsed.length) {
          blocks.push({ kind: 'html', html: renderSystemCheck('', parsed, ctx) });
          continue;
        }
      }
      if (kind === 'ol' && items.every(l => NUM_ITEM_RE.test(l))) {
        const groups = parseChoiceGroups(items);
        if (groups.some(g => g.options.length)) {
          blocks.push({ kind: 'choices', groups });
          continue;
        }
      }

      if (kind === 'task') {
        blocks.push({
          kind: 'html',
          html: `<div class="chat-task-list">${items.map(l => {
            const checked = /^\s*[•\-*]\s+\[[xX]\]/.test(l);
            const text = l.replace(TASK_RE, '');
            return `<div class="chat-task-item ${checked ? 'checked' : ''}"><span class="task-check-box">${checked ? icon('check', 11) : ''}</span><span class="task-text">${formatInline(text)}</span></div>`;
          }).join('')}</div>`,
        });
      } else if (kind === 'ol') {
        blocks.push({
          kind: 'html',
          html: `<ol class="chat-ordered-list">${items.map(l => {
            const m = l.match(/^\s*(\d+)[.)]\s+(.*)$/);
            return m
              ? `<li><span class="list-num">${m[1]}</span><span>${formatInline(m[2])}</span></li>`
              : `<li>${formatInline(l.trim())}</li>`;
          }).join('')}</ol>`,
        });
      } else {
        blocks.push({
          kind: 'html',
          html: `<ul class="chat-bullet-list">${items.map(l => `<li>${formatInline(l.replace(BULLET_RE, '').trim())}</li>`).join('')}</ul>`,
        });
      }
      continue;
    }

    // Paragraph
    const para = [];
    while (i < lines.length && lines[i].trim() && (para.length === 0 || !isBlockStart(lines, i))) {
      para.push(lines[i++].trim());
    }
    const text = para.join(' ');

    const squashed = splitInlineBullets(text);
    if (squashed) {
      if (squashed.lead) blocks.push({ kind: 'p', inline: formatInline(squashed.lead) });
      const asLines = squashed.items.map(t => `- ${t}`);
      if (asLines.every(l => KV_RE.test(l))) {
        blocks.push({ kind: 'html', html: renderSystemCheck('', parseKeyValueItems(asLines), ctx) });
      } else {
        blocks.push({
          kind: 'html',
          html: `<ul class="chat-bullet-list">${squashed.items.map(t => `<li>${formatInline(t)}</li>`).join('')}</ul>`,
        });
      }
      continue;
    }

    blocks.push({ kind: 'p', inline: formatInline(text) });
  }

  return blocks;
}

/* ------------------------------------------------------------------ */
/* Public API                                                          */
/* ------------------------------------------------------------------ */

/**
 * @param {string} rawText assistant markdown
 * @param {object} [opts]
 * @param {string}  [opts.msgId]        stable id, used for form/section keys
 * @param {boolean} [opts.interactive]  false for old messages: choices are read-only
 * @param {object}  [opts.selections]   saved radio selections keyed "formId:groupIdx"
 * @param {Set}     [opts.collapsed]    keys of collapsed system-check sections
 */
export function formatChatMessage(rawText, opts = {}) {
  if (!rawText) return '';

  const ctx = {
    msgId: String(opts.msgId ?? 'm'),
    interactive: opts.interactive !== false,
    selections: opts.selections || {},
    collapsed: opts.collapsed || new Set(),
    codeBlocks: [],
    counters: { sys: 0, form: 0 },
  };

  const normalized = String(rawText).replace(/\r\n/g, '\n');

  // Protect fenced code blocks
  const withoutCode = normalized.replace(/```([\w.\-]*)[ \t]*\n([\s\S]*?)```/g, (_, lang, code) => {
    ctx.codeBlocks.push({ lang: (lang || '').trim().toLowerCase(), code: code.replace(/\n$/, '') });
    return `\n__CODE_BLOCK_${ctx.codeBlocks.length - 1}__\n`;
  });

  const blocks = parseLines(withoutCode.split('\n'), ctx);

  const hasForm = blocks.some(b => b.kind === 'choices');
  const hasStructure = hasForm || blocks.some(b => b.kind === 'html' && /class="chat-(sys|sources|alert)/.test(b.html));
  const nextBlock = blocks.find(b => b.kind === 'next');

  let leadUsed = false;
  let nextConsumed = false;
  const out = [];

  blocks.forEach((b, idx) => {
    if (b.kind === 'p') {
      const lead = !leadUsed && idx === 0 && hasStructure;
      leadUsed = true;
      out.push(`<p class="chat-p${lead ? ' chat-lead' : ''}">${b.inline}</p>`);
    } else if (b.kind === 'choices') {
      const note = !nextConsumed && nextBlock ? nextBlock.text : '';
      if (note) nextConsumed = true;
      out.push(renderChoiceForm(b.groups, ctx, note));
    } else if (b.kind === 'next') {
      // Shown inside the choices form when there is one; otherwise as a quiet note.
      if (!hasForm && ctx.interactive) out.push(renderNextNote(b.text));
    } else {
      out.push(b.html);
    }
  });

  return out.join('');
}
