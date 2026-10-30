/**
 * SetupPal: modal dialog behaviour (focus trap, Esc to close, focus return).
 * Views rebuild their DOM on every store update, so bindModal runs after each render.
 */

const FOCUSABLE = [
  'button:not([disabled])',
  'a[href]',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  'summary',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

let openerId = null;

function focusables(root) {
  return [...root.querySelectorAll(FOCUSABLE)].filter(el => el.offsetParent !== null || el === document.activeElement);
}

/** Call right before opening a dialog so focus can go back to the control that opened it. */
export function rememberOpener() {
  openerId = document.activeElement?.id || null;
}

/** Call after a dialog closes and the view has re-rendered. */
export function restoreOpener() {
  const id = openerId;
  openerId = null;
  if (!id) return;
  requestAnimationFrame(() => document.getElementById(id)?.focus());
}

/**
 * @param {HTMLElement} dialog  element with role="dialog"
 * @param {object} opts
 * @param {() => void} opts.onClose
 * @param {HTMLElement} [opts.backdrop]  clicking it (not its children) closes the dialog
 */
export function bindModal(dialog, { onClose, backdrop = null }) {
  if (!dialog) return;

  dialog.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== 'Tab') return;
    const items = focusables(dialog);
    if (items.length === 0) {
      e.preventDefault();
      dialog.focus();
      return;
    }
    const first = items[0];
    const last = items[items.length - 1];
    if (e.shiftKey && (document.activeElement === first || !dialog.contains(document.activeElement))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  });

  if (backdrop) {
    backdrop.addEventListener('mousedown', (e) => {
      if (e.target === backdrop) onClose();
    });
  }

  if (!dialog.contains(document.activeElement)) {
    (focusables(dialog)[0] || dialog).focus();
  }
}

function escHtml(text) {
  return String(text ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/**
 * In-app replacement for window.confirm (native dialogs are blocked in the Tauri webview and
 * would block the UI thread). Mounted on <body>, so view re-renders never remove it.
 * Resolves true on confirm, false on cancel, Esc, or a backdrop click.
 */
export function confirmDialog({ title, message = '', confirmLabel = 'Confirm', cancelLabel = 'Cancel', danger = false }) {
  return new Promise((resolve) => {
    const previous = document.activeElement;
    const host = document.createElement('div');
    host.className = 'modal-backdrop';
    host.innerHTML = `
      <div class="modal-card confirm-card" role="dialog" aria-modal="true" aria-labelledby="confirm-title" tabindex="-1">
        <h2 class="modal-title" id="confirm-title">${escHtml(title)}</h2>
        ${message ? `<p class="confirm-message">${escHtml(message)}</p>` : ''}
        <div class="confirm-actions">
          <button type="button" class="btn-review" data-confirm="no">${escHtml(cancelLabel)}</button>
          <button type="button" class="btn-review confirm-yes${danger ? ' is-danger' : ''}" data-confirm="yes">${escHtml(confirmLabel)}</button>
        </div>
      </div>`;
    let settled = false;
    const finish = (value) => {
      if (settled) return;
      settled = true;
      host.remove();
      if (previous && typeof previous.focus === 'function' && document.contains(previous)) previous.focus();
      resolve(value);
    };
    host.querySelector('[data-confirm="yes"]').addEventListener('click', () => finish(true));
    host.querySelector('[data-confirm="no"]').addEventListener('click', () => finish(false));
    document.body.appendChild(host);
    bindModal(host.querySelector('[role="dialog"]'), { onClose: () => finish(false), backdrop: host });
    // Default focus on the safe choice.
    host.querySelector('[data-confirm="no"]').focus();
  });
}

export function isModalOpen() {
  return Boolean(document.querySelector('[role="dialog"][aria-modal="true"]'));
}
