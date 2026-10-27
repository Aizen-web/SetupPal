/**
 * SetupPal: Chat Message Markdown & Rich Layout Formatter
 * Transforms raw markdown into gorgeous, structured UI components:
 * - Pre-flight system diagnosis cards with status badges
 * - Guided setup instruction steps
 * - Modern callout/alert banners
 * - Styled verified checklists
 * - Inline code, blockquotes, bold/italic, and clean typography
 */

/**
 * Escapes HTML characters to prevent XSS.
 */
function escapeHtml(str) {
  if (!str) return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

/**
 * Formats inline Markdown (bold, italic, code pills, links).
 */
function formatInline(text) {
  if (!text) return '';

  let out = escapeHtml(text);

  // Inline code: `code`
  out = out.replace(/`([^`]+)`/g, '<code class="chat-code-pill">$1</code>');

  // Bold: **text**
  out = out.replace(/\*\*([^*]+)\*\*/g, '<strong class="chat-bold">$1</strong>');

  // Italic: *text* (avoiding already replaced tags)
  out = out.replace(/(^|[^*])\*([^*]+)\*([^*]|$)/g, '$1<em class="chat-italic">$2</em>$3');

  // Links: [text](url)
  out = out.replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer" class="chat-inline-link">$1 ↗</a>');

  return out;
}

/**
 * Parses and formats System Pre-Flight Diagnosis bullet lists into a rich spec grid.
 */
function parseDiagnosisBlock(lines) {
  const items = [];

  for (const line of lines) {
    const raw = line.trim();
    if (!raw.startsWith('•') && !raw.startsWith('-') && !raw.startsWith('*')) {
      continue;
    }

    const content = raw.replace(/^[•\-*]\s*/, '').trim();

    // Pattern: **Label**: Status / details
    const match = content.match(/^\*\*([^*]+)\*\*:\s*(.*)$/);
    if (match) {
      const label = match[1].trim();
      let details = match[2].trim();

      let statusType = 'neutral';
      let statusText = '';
      let cleanDetails = details;

      if (details.includes('❌') || details.toLowerCase().includes('not detected') || details.toLowerCase().includes('absent')) {
        statusType = 'missing';
        statusText = 'Not Detected';
      } else if (details.includes('⚠️') || details.toLowerCase().includes('not found') || details.toLowerCase().includes('caution')) {
        statusType = 'warn';
        statusText = 'Attention';
      } else if (details.includes('✅') || details.toLowerCase().includes('installed') || details.toLowerCase().includes('detected')) {
        statusType = 'ok';
        statusText = details.toLowerCase().includes('installed') ? 'Installed' : 'Detected';
      } else if (label.toLowerCase().includes('graphics') || label.toLowerCase().includes('gpu')) {
        statusType = 'ok';
        statusText = 'Active';
      } else if (label.toLowerCase().includes('zero-trust') || label.toLowerCase().includes('safety')) {
        statusType = 'ok';
        statusText = 'Enforced';
      }

      // Choose an icon based on the label
      let icon = '⚙️';
      const lower = label.toLowerCase();
      if (lower.includes('java edition')) icon = '🎮';
      else if (lower.includes('bedrock')) icon = '🧱';
      else if (lower.includes('java runtime') || lower.includes('runtime')) icon = '☕';
      else if (lower.includes('graphics') || lower.includes('gpu')) icon = '⚡';
      else if (lower.includes('zero-trust') || lower.includes('safety') || lower.includes('policy')) icon = '🛡️';

      // Clean up symbols in details
      cleanDetails = cleanDetails
        .replace(/^[✅❌⚠️]\s*/, '')
        .trim();

      if (cleanDetails.startsWith('Detected at ')) {
        cleanDetails = cleanDetails.replace(/^Detected at\s*/, '');
      } else if (cleanDetails.startsWith('Not detected (')) {
        cleanDetails = cleanDetails.replace(/^Not detected \((.*)\)$/, '$1');
      } else if (cleanDetails.startsWith('Installed (')) {
        cleanDetails = cleanDetails.replace(/^Installed \((.*)\)$/, '$1');
      }

      items.push({
        icon,
        label,
        statusType,
        statusText,
        details: cleanDetails
      });
    }
  }

  if (items.length === 0) return '';

  return `
    <div class="chat-diag-card">
      <div class="chat-diag-header">
        <div class="chat-diag-title-row">
          <span class="chat-diag-icon">🔍</span>
          <span class="chat-diag-title">Host Pre-Flight Diagnosis</span>
        </div>
        <span class="chat-diag-badge">Live System Inspection</span>
      </div>
      <div class="chat-diag-grid">
        ${items.map(it => `
          <div class="chat-diag-item ${it.statusType}">
            <div class="chat-diag-item-top">
              <span class="diag-item-icon">${it.icon}</span>
              <span class="diag-item-label">${escapeHtml(it.label)}</span>
              <span class="diag-status-pill ${it.statusType}">
                ${it.statusType === 'ok' ? '✓ ' + it.statusText : it.statusType === 'missing' ? '✕ ' + it.statusText : '⚠ ' + it.statusText}
              </span>
            </div>
            <div class="chat-diag-item-desc">${formatInline(it.details)}</div>
          </div>
        `).join('')}
      </div>
    </div>
  `;
}

/**
 * Formats a guided numbered steps block into sleek cards.
 */
function parseNumberedSteps(lines, headerTitle = 'Next Steps') {
  const steps = [];

  for (const line of lines) {
    const raw = line.trim();
    const match = raw.match(/^(\d+)\.\s+(.*)$/);
    if (match) {
      steps.push({
        num: match[1],
        text: match[2]
      });
    }
  }

  if (steps.length === 0) return '';

  return `
    <div class="chat-steps-card">
      ${headerTitle ? `
        <div class="chat-steps-header">
          <span class="chat-steps-icon">💡</span>
          <span class="chat-steps-title">${escapeHtml(headerTitle)}</span>
        </div>
      ` : ''}
      <div class="chat-steps-list">
        ${steps.map(s => `
          <div class="chat-step-item">
            <div class="chat-step-num">${s.num}</div>
            <div class="chat-step-text">${formatInline(s.text)}</div>
          </div>
        `).join('')}
      </div>
    </div>
  `;
}

/**
 * Main parser: converts raw markdown text into structured HTML.
 */
export function formatChatMessage(rawText) {
  if (!rawText) return '';

  const normalized = rawText.replace(/\r\n/g, '\n');
  const paragraphs = normalized.split(/\n{2,}/);
  const renderedBlocks = [];

  for (let i = 0; i < paragraphs.length; i++) {
    const p = paragraphs[i].trim();
    if (!p) continue;

    // 1. Alert Banner: ⚠️ **No AI API Key Found**
    if (p.includes('⚠️ **No AI API Key Found**') || p.startsWith('⚠️')) {
      const lines = p.split('\n');
      const titleLine = lines[0].replace(/[⚠️*]/g, '').trim() || 'Warning';
      const rest = lines.slice(1).join('\n').trim();

      renderedBlocks.push(`
        <div class="chat-alert-banner alert-warning">
          <div class="alert-icon-wrap">⚠️</div>
          <div class="alert-body">
            <div class="alert-title">${escapeHtml(titleLine)}</div>
            ${rest ? `<div class="alert-desc">${formatInline(rest)}</div>` : ''}
          </div>
        </div>
      `);
      continue;
    }

    // 2. Alert / Success Banner: ✅ **Zero-Trust Plan Synthesized**
    if (p.includes('✅ **Zero-Trust Plan Synthesized**') || p.startsWith('✅ **')) {
      const lines = p.split('\n');
      const titleLine = lines[0].replace(/[✅*]/g, '').trim();
      const rest = lines.slice(1).join('\n').trim();

      renderedBlocks.push(`
        <div class="chat-alert-banner alert-success">
          <div class="alert-icon-wrap">🛡️</div>
          <div class="alert-body">
            <div class="alert-title">${escapeHtml(titleLine)}</div>
            ${rest ? `<div class="alert-desc">${formatInline(rest)}</div>` : ''}
          </div>
        </div>
      `);
      continue;
    }

    // 3. System Pre-Flight Diagnosis Section
    if (p.includes('🔍 **Host System Pre-Flight Diagnosis**:') || p.includes('Pre-Flight Diagnosis')) {
      const lines = p.split('\n');
      renderedBlocks.push(parseDiagnosisBlock(lines));
      continue;
    }

    // 4. Guided Steps: 💡 **How to activate AI Planning**:
    if (p.includes('💡 **How to activate AI Planning**:') || (p.includes('💡') && /\n\d+\./.test(p))) {
      const lines = p.split('\n');
      const headerLine = lines[0].replace(/[💡*:]/g, '').trim();
      const stepLines = lines.slice(1);
      renderedBlocks.push(parseNumberedSteps(stepLines, headerLine || 'How to activate AI Planning'));
      continue;
    }

    // 5. Questions block: ❓ **Questions to formulate your verified Zero-Trust Plan**:
    if (p.includes('❓ **Questions') || p.includes('Questions to formulate')) {
      const lines = p.split('\n');
      const titleLine = lines[0].replace(/[❓*:]/g, '').trim();
      const restLines = lines.slice(1);

      // Format questions nicely
      renderedBlocks.push(`
        <div class="chat-questions-card">
          <div class="chat-questions-header">
            <span class="chat-questions-icon">❓</span>
            <span class="chat-questions-title">${escapeHtml(titleLine)}</span>
          </div>
          <div class="chat-questions-body">
            ${parseNumberedSteps(restLines, '')}
          </div>
        </div>
      `);
      continue;
    }

    // 6. Section Header with icon: 🎯 **Problem Definition & Scope**: or 📋 **Execution Summary**:
    if (/^[🎯📋🔍💡❓🛡️✅]\s*\*\*([^*]+)\*\*:?/.test(p)) {
      const lines = p.split('\n');
      const firstLine = lines[0];
      const match = firstLine.match(/^([🎯📋🔍💡❓🛡️✅])\s*\*\*([^*]+)\*\*:?\s*(.*)$/);
      if (match) {
        const icon = match[1];
        const title = match[2].trim();
        const inlineRemainder = match[3].trim();
        const restLines = lines.slice(1).join('\n').trim();

        // Check if restLines is a bullet list (like Execution Summary)
        if (restLines.startsWith('•') || restLines.startsWith('-')) {
          const bulletItems = restLines.split('\n').filter(l => l.trim().startsWith('•') || l.trim().startsWith('-'));
          renderedBlocks.push(`
            <div class="chat-summary-card">
              <div class="chat-summary-header">
                <span class="summary-icon">${icon}</span>
                <span class="summary-title">${escapeHtml(title)}</span>
              </div>
              <div class="chat-summary-list">
                ${bulletItems.map(item => {
                  const cleaned = item.replace(/^[•\-]\s*/, '').trim();
                  return `<div class="summary-item"><span class="summary-check">✓</span><span>${formatInline(cleaned)}</span></div>`;
                }).join('')}
              </div>
            </div>
          `);
          continue;
        }

        renderedBlocks.push(`
          <div class="chat-section-block">
            <div class="chat-section-header">
              <span class="chat-section-icon">${icon}</span>
              <span class="chat-section-title">${escapeHtml(title)}</span>
            </div>
            ${inlineRemainder ? `<p class="chat-p">${formatInline(inlineRemainder)}</p>` : ''}
            ${restLines ? `<p class="chat-p">${formatInline(restLines)}</p>` : ''}
          </div>
        `);
        continue;
      }
    }

    // 7. Tip Note / Offline Recipe Note: *In the meantime, you can test one of our verified offline recipes...*
    if (p.startsWith('*') && p.endsWith('*') && p.length > 2) {
      const inner = p.slice(1, -1).trim();
      renderedBlocks.push(`
        <div class="chat-tip-pill">
          <span class="chat-tip-sparkle">✨</span>
          <span class="chat-tip-text">${formatInline(inner)}</span>
        </div>
      `);
      continue;
    }

    // 8. Blockquote: > 💡 ...
    if (p.startsWith('>')) {
      const clean = p.replace(/^>\s*/gm, '').trim();
      renderedBlocks.push(`
        <div class="chat-callout-pill">
          <span class="callout-icon">💡</span>
          <div class="callout-text">${formatInline(clean)}</div>
        </div>
      `);
      continue;
    }

    // 9. Numbered list fallback: 1. ... 2. ...
    if (/^\d+\.\s+/.test(p)) {
      renderedBlocks.push(parseNumberedSteps(p.split('\n'), ''));
      continue;
    }

    // 10. Bullet list fallback: • ... or - ...
    if (/^[•\-*]\s+/.test(p)) {
      const items = p.split('\n').filter(l => l.trim().length > 0);
      renderedBlocks.push(`
        <ul class="chat-bullet-list">
          ${items.map(it => `<li>${formatInline(it.replace(/^[•\-*]\s*/, ''))}</li>`).join('')}
        </ul>
      `);
      continue;
    }

    // 11. Code blocks: ```...```
    if (p.startsWith('```')) {
      const cleanCode = p.replace(/^```[a-z]*\n?/i, '').replace(/```$/, '');
      renderedBlocks.push(`
        <pre class="chat-code-block"><code>${escapeHtml(cleanCode)}</code></pre>
      `);
      continue;
    }

    // 12. Standard Paragraph
    renderedBlocks.push(`<p class="chat-p">${formatInline(p)}</p>`);
  }

  return renderedBlocks.join('');
}
