/**
 * SetupPal: Mascot & Pets Animation Lab (20 Full Animations)
 * Interactive tester for Pixel (Ghost), Byte (Cat), and Mochi (Bunny)
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';
import { createMascotSvg } from '../mascot/pets.js';
import confetti from 'canvas-confetti';

export function renderAnimationLab(container) {
  const state = store.getState();
  if (!state.animationLabOpen) {
    container.innerHTML = '';
    return;
  }

  const activePet = state.activePet || 'ghost';

  const animations = [
    { id: 'idle', label: '1. Idle Float', desc: 'Gentle float + shadow & tail/ear sways', sound: 'ui.click' },
    { id: 'blink', label: '2. Blink', desc: 'Eye blink scaleY animation', sound: 'ui.click' },
    { id: 'speaking', label: '3. Speaking', desc: 'Mouth squashes with typing text', sound: 'msg.receive' },
    { id: 'thinking', label: '4. Thinking', desc: 'Head tilted, 3 pulsing thought dots', sound: 'ui.click' },
    { id: 'scanning', label: '5. Scanning', desc: 'Flashlight beam sweeps left-right', sound: 'ui.click' },
    { id: 'working', label: '6. Working', desc: 'Hardhat on, rapid float, sparkles / typing paws', sound: 'task.start' },
    { id: 'glasses-on', label: '7. Glasses On', desc: 'Hacker sunglasses slide on with bounce', sound: 'mode.hacker' },
    { id: 'glasses-off', label: '8. Glasses Off', desc: 'Sunglasses slide away', sound: 'mode.chill' },
    { id: 'green', label: '9. Green Verdict', desc: 'Happy eyes, sparkle burst, joyful hop', sound: 'verdict.green' },
    { id: 'yellow', label: '10. Yellow / Held', desc: 'Head tilt 8°, sweat drop, worried brow', sound: 'verdict.yellow' },
    { id: 'red', label: '11. Red / Blocked', desc: 'Sad crying face, streaming tears, shield pop', sound: 'verdict.red' },
    { id: 'celebrate', label: '12. Celebrate', desc: '360° spin, confetti shower, victory', sound: 'celebrate' },
    { id: 'wave-hello', label: '13. Wave Hello', desc: 'Playful left & right welcoming wave', sound: 'msg.send' },
    { id: 'listening', label: '14. Listening', desc: 'Leans forward toward user, eyes widen', sound: 'ui.click' },
    { id: 'downloading', label: '15. Downloading', desc: 'Carries package box, speed ripple hover', sound: 'download.done' },
    { id: 'error', label: '16. Error (Crying)', desc: 'Sad mouth, droops, shedding animated tears', sound: 'error' },
    { id: 'held-sign', label: '17. Held Sign', desc: 'Holds up yellow warning caution sign', sound: 'verdict.yellow' },
    { id: 'rewind', label: '18. Rewind', desc: 'Reverse spin, backwards clock hands', sound: 'rewind' },
    { id: 'sleepy', label: '19. Sleepy', desc: 'Half-closed eyes, floating Zzz bubbles', sound: 'ui.click' },
    { id: 'peek-a-boo', label: '20. Peek-A-Boo', desc: 'Slides in from edge, peeks, then centers', sound: 'task.done' }
  ];

  container.innerHTML = `
    <div class="modal-backdrop" id="lab-backdrop">
      <div class="modal-card" style="max-width: 680px;">
        <div class="modal-header">
          <div class="modal-title">Kawaii Mascot Lab (20 Animations)</div>
          <button class="btn-close-modal" id="btn-close-lab">✕</button>
        </div>

        <!-- Pet Selector Tabs -->
        <div style="display: flex; align-items: center; justify-content: space-between; gap: 8px; margin: 4px 0;">
          <div style="display: flex; gap: 6px;">
            <button class="mode-btn ${activePet === 'ghost' ? 'active' : ''}" data-select-pet="ghost">👻 Pixel the Ghost</button>
            <button class="mode-btn ${activePet === 'cat' ? 'active' : ''}" data-select-pet="cat">🐱 Byte the Cat</button>
            <button class="mode-btn ${activePet === 'bunny' ? 'active' : ''}" data-select-pet="bunny">🐰 Mochi the Bunny</button>
          </div>
          <div style="font-size: 12px; color: var(--ink-soft);">
            Total: <strong>20 Animations</strong>
          </div>
        </div>

        <!-- Mascot Live Preview -->
        <div style="display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 20px 0; background: var(--bg-canvas); border-radius: var(--radius-md); border: 1px solid var(--border-subtle);">
          <div style="width: 150px; height: 150px; display: flex; align-items: center; justify-content: center;" id="lab-mascot-box">
            ${createMascotSvg(activePet, 140, 'lab')}
          </div>
          <div style="margin-top: 10px; font-weight: 700; font-size: 14px; color: var(--accent-strong);" id="lab-status-text">
            Pet: ${activePet.toUpperCase()} · Active: ${state.mascotState.toUpperCase()}
          </div>
        </div>

        <!-- 20 Animations Grid -->
        <div class="lab-grid" style="grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));">
          ${animations.map(a => `
            <button class="btn-lab-state" data-anim="${a.id}" data-sound="${a.sound}">
              <span>${a.label}</span>
              <span style="font-size: 10px; font-weight: 400; color: var(--ink-soft);">${a.desc}</span>
            </button>
          `).join('')}
        </div>
      </div>
    </div>
  `;

  const labSvg = container.querySelector('#lab-ghost-root');
  if (labSvg) {
    labSvg.dataset.state = state.mascotState;
    labSvg.dataset.mode = state.mode;
  }

  // Pet selection
  container.querySelectorAll('[data-select-pet]').forEach(btn => {
    btn.onclick = () => {
      const pet = btn.getAttribute('data-select-pet');
      store.setActivePet(pet);
    };
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

      if (anim === 'glasses-on') {
        store.setMode('hacker');
        if (labSvg) {
          labSvg.dataset.mode = 'hacker';
          labSvg.dataset.glasses = 'true';
        }
        return;
      }
      if (anim === 'glasses-off') {
        store.setMode('chill');
        if (labSvg) {
          labSvg.dataset.mode = 'chill';
          labSvg.dataset.glasses = 'false';
        }
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
      if (labSvg) labSvg.dataset.state = anim;
      const statusText = container.querySelector('#lab-status-text');
      if (statusText) statusText.innerText = `Pet: ${activePet.toUpperCase()} · Active: ${anim.toUpperCase()}`;
    };
  });
}
