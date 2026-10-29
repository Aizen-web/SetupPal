/**
 * SetupPal: UI run controls.
 * No simulated flows; this only resets the UI when the user presses Stop / Esc.
 */

import { store } from './store.js';
import { sounds } from '../audio/sound-engine.js';
import { petVoiceEngine } from '../mascot/voicelines.js';

class SimulationEngine {
  stopRun() {
    store.setMode('chill');
    store.setMascotState('idle', petVoiceEngine.line('stopped'));
    sounds.play('mode.chill');
  }
}

export const simulation = new SimulationEngine();
