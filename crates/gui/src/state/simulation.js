/**
 * SetupPal: UI run controls.
 * No simulated flows; this only resets the UI when the user presses Stop / Esc.
 */

import { store } from './store.js';
import { sounds } from '../audio/sound-engine.js';
import { pixelVoice } from '../mascot/voicelines.js';

class SimulationEngine {
  stopRun() {
    const running = store.runActive
      || Boolean(store.getState().activePlan?.tasks?.some(t => t.status === 'running'));
    if (!running) return false;
    store.endRun();
    store.setMascotState('idle', pixelVoice.line('stopped'));
    sounds.play('mode.chill');
    return true;
  }
}

export const simulation = new SimulationEngine();
