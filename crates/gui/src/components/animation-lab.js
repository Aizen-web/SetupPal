/**
 * SetupPal: Mascot & Pets Animation Lab (20 Full Animations)
 * Interactive tester for Pixel (Ghost), Byte (Cat), and Mochi (Bunny)
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { createMascotSvg } from '../mascot/pets.js';
import { ANIMATIONS } from '../mascot/animations.js';
import { icon } from './icon.js';
import confetti from 'canvas-confetti';

const labPlayback = { speed: 1, loop: true };

export function renderAnimationLab(container) {
  const state = store.getState();
  if (!state.animationLabOpen) {
    container.innerHTML = '';
    return;
  }

  const activePet = state.activePet || 'ghost';

  container.innerHTML = `
    <div class="modal-backdrop" id="lab-backdrop">
      <div class="modal-card mascot-lab-card">
        <div class="modal-header">
          <div>
            <div class="modal-title">Mascot Lab (${ANIMATIONS.length} ${ANIMATIONS.length === 1 ? 'animation' : 'animations'})</div>
            <div class="lab-coverage">${ANIMATIONS.length} registered states · 3 selectable mascots</div>
          </div>
          <button class="btn-close-modal" id="btn-close-lab" aria-label="Close Mascot Lab">${icon('close')}</button>
        </div>

        <div class="lab-controls">
          <div class="lab-pet-selector">
            <button class="mode-btn ${activePet === 'ghost' ? 'active' : ''}" data-select-pet="ghost">${icon('ghost', 18)}<span>Pixel</span></button>
            <button class="mode-btn ${activePet === 'cat' ? 'active' : ''}" data-select-pet="cat">${icon('cat', 18)}<span>Byte</span></button>
            <button class="mode-btn ${activePet === 'bunny' ? 'active' : ''}" data-select-pet="bunny">${icon('bunny', 18)}<span>Mochi</span></button>
          </div>
          <div class="mode-toggle-group">
            <button class="mode-btn ${state.mode === 'chill' ? 'active' : ''}" data-lab-mode="chill">Chill</button>
            <button class="mode-btn ${state.mode === 'hacker' ? 'active' : ''}" data-lab-mode="hacker">Hacker</button>
          </div>
        </div>

        <div class="lab-playback-controls" aria-label="Animation playback controls">
          <div class="lab-speed-controls" role="group" aria-label="Animation speed">
            ${[0.25, 0.5, 1].map((speed) => `
              <button class="lab-speed-btn ${labPlayback.speed === speed ? 'active' : ''}" type="button" data-speed="${speed}" aria-pressed="${labPlayback.speed === speed}">
                ${speed}×
              </button>
            `).join('')}
          </div>
          <button class="lab-loop-btn" type="button" data-loop aria-pressed="${labPlayback.loop}">
            Loop: ${labPlayback.loop ? 'On' : 'Off'}
          </button>
        </div>

        <!-- Mascot Live Preview -->
        <div class="lab-preview">
          <div class="lab-mascot-box" id="lab-mascot-box">
            ${createMascotSvg(activePet, 140, 'lab')}
          </div>
          <div class="lab-status-text" id="lab-status-text">
            ${activePet.toUpperCase()} · ${state.mascotState.toUpperCase()}
          </div>
        </div>

        <div class="lab-grid">
          ${ANIMATIONS.map((a) => `
            <button class="btn-lab-state ${state.mascotState === a.id ? 'active' : ''}" data-anim="${a.id}" data-sound="${a.sound}" aria-label="Replay ${a.label}">
              <span class="lab-animation-name">${a.label}</span>
              <span class="lab-animation-meta">${(a.durationMs / 1000).toFixed(1)} s · ${a.loop ? 'loop' : 'one-shot'}</span>
              <span class="lab-implemented">${icon('check', 14)} State registered</span>
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
    if (state.mode === 'hacker' || state.mascotState === 'glasses-on') {
      labSvg.dataset.glasses = 'true';
    }
    applyPlaybackSettings(labSvg);
  }

  container.querySelectorAll('[data-speed]').forEach((btn) => {
    btn.onclick = () => {
      labPlayback.speed = Number(btn.getAttribute('data-speed'));
      const currentLabSvg = getLabSvg();
      if (currentLabSvg) applyPlaybackSettings(currentLabSvg);
      renderAnimationLab(container);
    };
  });

  container.querySelector('[data-loop]').onclick = () => {
    labPlayback.loop = !labPlayback.loop;
    const currentLabSvg = getLabSvg();
    if (currentLabSvg) applyPlaybackSettings(currentLabSvg);
    renderAnimationLab(container);
  };

  // Pet selection
  container.querySelectorAll('[data-select-pet]').forEach(btn => {
    btn.onclick = () => {
      const pet = btn.getAttribute('data-select-pet');
      store.setActivePet(pet);
    };
  });

  container.querySelectorAll('[data-lab-mode]').forEach((btn) => {
    btn.onclick = () => store.setMode(btn.getAttribute('data-lab-mode'));
  });

  // Close Lab
  container.querySelector('#btn-close-lab').onclick = () => {
    store.toggleAnimationLab(false);
  };
  container.querySelector('#lab-backdrop').onclick = (e) => {
    if (e.target.id === 'lab-backdrop') {
      store.toggleAnimationLab(false);
    }
  };

  // Button triggers for 20 animations
  container.querySelectorAll('.btn-lab-state').forEach(btn => {
    btn.onclick = () => {
      const anim = btn.getAttribute('data-anim');
      const sound = btn.getAttribute('data-sound');
      if (sound) sounds.play(sound);

      // Highlight active button
      container.querySelectorAll('.btn-lab-state').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');

      const currentLabSvg = getLabSvg();

      if (anim === 'glasses-on') {
        store.setMode('hacker');
        store.setMascotState('glasses-on');
        if (currentLabSvg) {
          currentLabSvg.dataset.mode = 'hacker';
          currentLabSvg.dataset.glasses = 'true';
          currentLabSvg.dataset.state = 'glasses-on';
        }
        const statusText = container.querySelector('#lab-status-text');
        if (statusText) statusText.innerText = `Pet: ${activePet.toUpperCase()} · Active: GLASSES-ON`;
        return;
      }

      if (anim === 'glasses-off') {
        store.setMode('chill');
        store.setMascotState('glasses-off');
        if (currentLabSvg) {
          currentLabSvg.dataset.mode = 'chill';
          currentLabSvg.dataset.glasses = 'false';
          currentLabSvg.dataset.state = 'glasses-off';
        }
        const statusText = container.querySelector('#lab-status-text');
        if (statusText) statusText.innerText = `Pet: ${activePet.toUpperCase()} · Active: GLASSES-OFF`;
        return;
      }

      if (anim === 'celebrate') {
        try {
          confetti({
            particleCount: 50,
            spread: 50,
            origin: { y: 0.5 }
          });
        } catch (e) {}
      }

      store.setMascotState(anim);

      if (currentLabSvg) {
        // Force reflow so animation restarts cleanly even when clicked repeatedly
        currentLabSvg.dataset.state = '';
        void currentLabSvg.offsetWidth;
        currentLabSvg.dataset.state = anim;
        applyPlaybackSettings(currentLabSvg);
      }

      const statusText = container.querySelector('#lab-status-text');
      if (statusText) statusText.innerText = `Pet: ${activePet.toUpperCase()} · Active: ${anim.toUpperCase()}`;
    };
  });
}

function applyPlaybackSettings(svg) {
  for (const animation of svg.getAnimations({ subtree: true })) {
    animation.playbackRate = labPlayback.speed;
    animation.effect.updateTiming({ iterations: labPlayback.loop ? Infinity : 1 });
  }
}
