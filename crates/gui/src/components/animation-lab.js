/**
 * SetupPal: Mascot Lab
 * Preview every registered Pixel animation with speed and loop controls.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { createPixelSvg } from '../mascot/pixel.js';
import { ANIMATIONS } from '../mascot/animations.js';
import { motionReduced } from '../state/prefs.js';
import { esc } from './html.js';
import { icon } from './icon.js';
import { bindModal, restoreOpener } from './modal.js';
import confetti from 'canvas-confetti';

const labPlayback = { speed: 1, loop: true };

function closeLab() {
  store.toggleAnimationLab(false);
  restoreOpener();
}

export function renderAnimationLab(container) {
  const state = store.getState();
  if (!state.animationLabOpen) {
    container.innerHTML = '';
    return;
  }

  container.innerHTML = `
    <div class="modal-backdrop" id="lab-backdrop">
      <div class="modal-card mascot-lab-card" role="dialog" aria-modal="true" aria-labelledby="lab-title" tabindex="-1">
        <div class="modal-header">
          <div>
            <h2 class="modal-title" id="lab-title">Mascot Lab</h2>
            <div class="lab-coverage">${ANIMATIONS.length} ${ANIMATIONS.length === 1 ? 'animation' : 'animations'} registered for Pixel</div>
          </div>
          <button type="button" class="btn-close-modal" id="btn-close-lab" aria-label="Close Mascot Lab">${icon('close')}</button>
        </div>

        <div class="lab-controls">
          <div class="mode-toggle-group" role="group" aria-label="Preview mode">
            <button type="button" id="lab-mode-chill" class="mode-btn ${state.mode === 'chill' ? 'active' : ''}" data-lab-mode="chill" aria-pressed="${state.mode === 'chill'}">Chill</button>
            <button type="button" id="lab-mode-hacker" class="mode-btn ${state.mode === 'hacker' ? 'active' : ''}" data-lab-mode="hacker" aria-pressed="${state.mode === 'hacker'}">Hacker</button>
          </div>
          <div class="lab-playback-controls">
            <div class="lab-speed-controls" role="group" aria-label="Animation speed">
              ${[0.25, 0.5, 1].map((speed) => `
                <button type="button" id="lab-speed-${String(speed).replace('.', '-')}" class="lab-speed-btn ${labPlayback.speed === speed ? 'active' : ''}" data-speed="${speed}" aria-pressed="${labPlayback.speed === speed}">${speed}×</button>
              `).join('')}
            </div>
            <button type="button" id="lab-loop" class="lab-loop-btn" data-loop aria-pressed="${labPlayback.loop}">
              Loop: ${labPlayback.loop ? 'On' : 'Off'}
            </button>
          </div>
        </div>

        <div class="lab-preview">
          <div class="lab-mascot-box" id="lab-mascot-box">
            ${createPixelSvg(140, 'lab')}
          </div>
          <div class="lab-status-text" id="lab-status-text" aria-live="polite">Pixel · ${esc(state.mascotState)}</div>
        </div>

        <div class="lab-grid">
          ${ANIMATIONS.map((a) => `
            <button type="button" id="lab-anim-${esc(a.id)}" class="btn-lab-state ${state.mascotState === a.id ? 'active' : ''}" data-anim="${esc(a.id)}" data-sound="${esc(a.sound)}" aria-pressed="${state.mascotState === a.id}">
              <span class="lab-animation-name">${esc(a.label)}</span>
              <span class="lab-animation-meta">${(a.durationMs / 1000).toFixed(1)} s · ${a.loop ? 'loop' : 'one-shot'}</span>
            </button>
          `).join('')}
        </div>
      </div>
    </div>
  `;

  const getLabSvg = () => container.querySelector('#lab-ghost-root');
  const labSvg = getLabSvg();
  if (labSvg) {
    labSvg.dataset.state = state.mascotState;
    labSvg.dataset.mode = state.mode;
    if (state.mode === 'hacker' || state.mascotState === 'glasses-on') labSvg.dataset.glasses = 'true';
    applyPlaybackSettings(labSvg);
  }

  bindModal(container.querySelector('[role="dialog"]'), {
    onClose: closeLab,
    backdrop: container.querySelector('#lab-backdrop'),
  });
  container.querySelector('#btn-close-lab').onclick = closeLab;

  container.querySelectorAll('[data-speed]').forEach((btn) => {
    btn.onclick = () => {
      labPlayback.speed = Number(btn.getAttribute('data-speed'));
      renderAnimationLab(container);
      document.getElementById(btn.id)?.focus();
    };
  });

  container.querySelector('[data-loop]').onclick = () => {
    labPlayback.loop = !labPlayback.loop;
    renderAnimationLab(container);
    document.getElementById('lab-loop')?.focus();
  };

  container.querySelectorAll('[data-lab-mode]').forEach((btn) => {
    btn.onclick = () => store.setModeManual(btn.getAttribute('data-lab-mode'));
  });

  // setMascotState updates the DOM in place (no re-render), so the lab mirrors it here.
  const showInLab = (anim) => {
    container.querySelectorAll('.btn-lab-state').forEach(b => {
      const on = b.getAttribute('data-anim') === anim;
      b.classList.toggle('active', on);
      b.setAttribute('aria-pressed', String(on));
    });
    const svg = getLabSvg();
    if (svg) {
      svg.dataset.mode = store.getState().mode;
      svg.dataset.glasses = String(anim === 'glasses-on' || store.getState().mode === 'hacker');
      svg.dataset.state = '';
      void svg.offsetWidth;
      svg.dataset.state = anim;
      applyPlaybackSettings(svg);
    }
    const status = container.querySelector('#lab-status-text');
    if (status) status.textContent = `Pixel · ${anim}`;
  };

  container.querySelectorAll('.btn-lab-state').forEach(btn => {
    btn.onclick = () => {
      const anim = btn.getAttribute('data-anim');
      const sound = btn.getAttribute('data-sound');
      if (sound) sounds.play(sound);

      if (anim === 'glasses-on') store.setModeManual('hacker');
      if (anim === 'glasses-off') store.setModeManual('chill');
      if (anim === 'celebrate' && !motionReduced()) {
        try {
          confetti({ particleCount: 50, spread: 50, origin: { y: 0.5 } });
        } catch (e) {}
      }
      store.setMascotState(anim);
      showInLab(anim);
    };
  });
}

function applyPlaybackSettings(svg) {
  for (const animation of svg.getAnimations({ subtree: true })) {
    animation.playbackRate = labPlayback.speed;
    animation.effect.updateTiming({ iterations: labPlayback.loop ? Infinity : 1 });
  }
}
