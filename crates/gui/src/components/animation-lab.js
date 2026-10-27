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
    { id: 'blink', label: '2. Blink', desc: 'Rapid cute anime eye blink cycle', sound: 'ui.click' },
    { id: 'speaking', label: '3. Speaking', desc: 'Mouth squashes with rhythmic body bob', sound: 'msg.receive' },
    { id: 'thinking', label: '4. Thinking', desc: 'Thoughtful float, 3 pulsing thought dots', sound: 'ui.click' },
    { id: 'scanning', label: '5. Scanning', desc: 'Flashlight beam sweeps left-right', sound: 'ui.click' },
    { id: 'working', label: '6. Working', desc: 'Hardhat on, rapid float, sparkles / typing paws', sound: 'task.start' },
    { id: 'glasses-on', label: '7. Glasses On', desc: 'Hacker sunglasses on with lens glint sweep', sound: 'mode.hacker' },
    { id: 'glasses-off', label: '8. Glasses Off', desc: 'Sunglasses slide away', sound: 'mode.chill' },
    { id: 'green', label: '9. Green Verdict', desc: 'Happy eyes, sparkle burst, joyful hop loop', sound: 'verdict.green' },
    { id: 'yellow', label: '10. Yellow / Held', desc: 'Worried tilt float, sweat drop, worried brow', sound: 'verdict.yellow' },
    { id: 'red', label: '11. Red / Blocked', desc: 'Sad crying face, streaming tears, shield pulse', sound: 'verdict.red' },
    { id: 'celebrate', label: '12. Celebrate', desc: '360° victory spin loop, confetti shower', sound: 'celebrate' },
    { id: 'wave-hello', label: '13. Wave Hello', desc: 'Playful left & right welcoming wave loop', sound: 'msg.send' },
    { id: 'listening', label: '14. Listening', desc: 'Attentive lean hover, eyes widen', sound: 'ui.click' },
    { id: 'downloading', label: '15. Downloading', desc: 'Carries package box, speed ripple hover', sound: 'download.done' },
    { id: 'error', label: '16. Error (Crying)', desc: 'Sad mouth, drooping bob, shedding animated tears', sound: 'error' },
    { id: 'held-sign', label: '17. Held Sign', desc: 'Holds up yellow warning caution sign', sound: 'verdict.yellow' },
    { id: 'rewind', label: '18. Rewind', desc: 'Reverse spin, backwards clock hands', sound: 'rewind' },
    { id: 'sleepy', label: '19. Sleepy', desc: 'Drowsy slow float, floating Zzz bubbles', sound: 'ui.click' },
    { id: 'peek-a-boo', label: '20. Peek-A-Boo', desc: 'Playful peek in and out hide-and-seek loop', sound: 'task.done' }
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
            <button class="btn-lab-state ${state.mascotState === a.id ? 'active' : ''}" data-anim="${a.id}" data-sound="${a.sound}">
              <span>${a.label}</span>
              <span style="font-size: 10px; font-weight: 400; color: var(--ink-soft);">${a.desc}</span>
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
      }

      const statusText = container.querySelector('#lab-status-text');
      if (statusText) statusText.innerText = `Pet: ${activePet.toUpperCase()} · Active: ${anim.toUpperCase()}`;
    };
  });
}
