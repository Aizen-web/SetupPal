/**
 * SetupPal: Cozy 8-Bit Web Audio Sound Engine
 * Implements audio event map from ui.md Section 14
 */

class SoundEngine {
  constructor() {
    this.ctx = null;
    this.masterGain = null;
    this.enabled = true;
    this.volume = 0.35;
    this.lastPlayed = {};
    this.initialized = false;
  }

  init() {
    if (this.initialized) return;
    try {
      const AudioCtx = window.AudioContext || window.webkitAudioContext;
      if (!AudioCtx) return;
      this.ctx = new AudioCtx();
      this.masterGain = this.ctx.createGain();
      this.masterGain.gain.setValueAtTime(this.volume, this.ctx.currentTime);
      this.masterGain.connect(this.ctx.destination);
      this.initialized = true;
    } catch (e) {
      console.warn("AudioContext init postponed until user gesture", e);
    }
  }

  setVolume(vol) {
    this.volume = Math.max(0, Math.min(1, vol));
    if (this.masterGain && this.ctx) {
      this.masterGain.gain.setValueAtTime(this.enabled ? this.volume : 0, this.ctx.currentTime);
    }
  }

  setMuted(muted) {
    this.enabled = !muted;
    if (this.masterGain && this.ctx) {
      this.masterGain.gain.setValueAtTime(this.enabled ? this.volume : 0, this.ctx.currentTime);
    }
  }

  play(eventId) {
    if (!this.enabled) return;
    if (!this.initialized) {
      this.init();
    }
    if (!this.ctx) return;
    if (this.ctx.state === 'suspended') {
      this.ctx.resume();
    }

    // Rate limit: prevent same sound within 120ms
    const now = performance.now();
    if (this.lastPlayed[eventId] && now - this.lastPlayed[eventId] < 120) {
      return;
    }
    this.lastPlayed[eventId] = now;

    try {
      const t = this.ctx.currentTime;
      switch (eventId) {
        case 'ui.click': {
          const osc = this.ctx.createOscillator();
          const gain = this.ctx.createGain();
          osc.type = 'triangle';
          osc.frequency.setValueAtTime(800, t);
          osc.frequency.exponentialRampToValueAtTime(300, t + 0.05);
          gain.gain.setValueAtTime(0.2, t);
          gain.gain.exponentialRampToValueAtTime(0.001, t + 0.05);
          osc.connect(gain);
          gain.connect(this.masterGain);
          osc.start(t);
          osc.stop(t + 0.05);
          break;
        }

        case 'msg.send': {
          const osc = this.ctx.createOscillator();
          const gain = this.ctx.createGain();
          osc.type = 'sine';
          osc.frequency.setValueAtTime(440, t);
          osc.frequency.exponentialRampToValueAtTime(720, t + 0.1);
          gain.gain.setValueAtTime(0.25, t);
          gain.gain.exponentialRampToValueAtTime(0.001, t + 0.12);
          osc.connect(gain);
          gain.connect(this.masterGain);
          osc.start(t);
          osc.stop(t + 0.12);
          break;
        }

        case 'msg.receive': {
          // Two soft notes
          [620, 880].forEach((freq, i) => {
            const osc = this.ctx.createOscillator();
            const gain = this.ctx.createGain();
            const noteStart = t + i * 0.09;
            osc.type = 'sine';
            osc.frequency.setValueAtTime(freq, noteStart);
            gain.gain.setValueAtTime(0.2, noteStart);
            gain.gain.exponentialRampToValueAtTime(0.001, noteStart + 0.08);
            osc.connect(gain);
            gain.connect(this.masterGain);
            osc.start(noteStart);
            osc.stop(noteStart + 0.08);
          });
          break;
        }

        case 'mode.hacker': {
          // Cyber retro synth sweep up
          const osc = this.ctx.createOscillator();
          const gain = this.ctx.createGain();
          osc.type = 'sawtooth';
          osc.frequency.setValueAtTime(140, t);
          osc.frequency.exponentialRampToValueAtTime(640, t + 0.28);
          gain.gain.setValueAtTime(0.22, t);
          gain.gain.exponentialRampToValueAtTime(0.001, t + 0.35);
          osc.connect(gain);
          gain.connect(this.masterGain);
          osc.start(t);
          osc.stop(t + 0.35);
          break;
        }

        case 'mode.chill': {
          // Cozy warm sweep down
          const osc = this.ctx.createOscillator();
          const gain = this.ctx.createGain();
          osc.type = 'sine';
          osc.frequency.setValueAtTime(580, t);
          osc.frequency.exponentialRampToValueAtTime(260, t + 0.25);
          gain.gain.setValueAtTime(0.25, t);
          gain.gain.exponentialRampToValueAtTime(0.001, t + 0.3);
          osc.connect(gain);
          gain.connect(this.masterGain);
          osc.start(t);
          osc.stop(t + 0.3);
          break;
        }

        case 'task.done':
        case 'verdict.green': {
          // Bright happy ding
          [523.25, 659.25, 783.99].forEach((freq, idx) => {
            const osc = this.ctx.createOscillator();
            const gain = this.ctx.createGain();
            const noteStart = t + idx * 0.07;
            osc.type = 'triangle';
            osc.frequency.setValueAtTime(freq, noteStart);
            gain.gain.setValueAtTime(0.24, noteStart);
            gain.gain.exponentialRampToValueAtTime(0.001, noteStart + 0.2);
            osc.connect(gain);
            gain.connect(this.masterGain);
            osc.start(noteStart);
            osc.stop(noteStart + 0.2);
          });
          break;
        }

        case 'verdict.yellow': {
          // Curious two-note "hmm"
          [440, 415].forEach((freq, idx) => {
            const osc = this.ctx.createOscillator();
            const gain = this.ctx.createGain();
            const noteStart = t + idx * 0.12;
            osc.type = 'sine';
            osc.frequency.setValueAtTime(freq, noteStart);
            gain.gain.setValueAtTime(0.22, noteStart);
            gain.gain.exponentialRampToValueAtTime(0.001, noteStart + 0.15);
            osc.connect(gain);
            gain.connect(this.masterGain);
            osc.start(noteStart);
            osc.stop(noteStart + 0.15);
          });
          break;
        }

        case 'verdict.red':
        case 'error': {
          // Low warning buzz
          const osc = this.ctx.createOscillator();
          const gain = this.ctx.createGain();
          osc.type = 'sawtooth';
          osc.frequency.setValueAtTime(120, t);
          osc.frequency.setValueAtTime(90, t + 0.15);
          gain.gain.setValueAtTime(0.3, t);
          gain.gain.exponentialRampToValueAtTime(0.001, t + 0.4);
          osc.connect(gain);
          gain.connect(this.masterGain);
          osc.start(t);
          osc.stop(t + 0.4);
          break;
        }

        case 'task.all_done':
        case 'celebrate': {
          // Cozy victory arpeggio
          [440, 554.37, 659.25, 880].forEach((freq, idx) => {
            const osc = this.ctx.createOscillator();
            const gain = this.ctx.createGain();
            const noteStart = t + idx * 0.09;
            osc.type = 'triangle';
            osc.frequency.setValueAtTime(freq, noteStart);
            gain.gain.setValueAtTime(0.26, noteStart);
            gain.gain.exponentialRampToValueAtTime(0.001, noteStart + 0.35);
            osc.connect(gain);
            gain.connect(this.masterGain);
            osc.start(noteStart);
            osc.stop(noteStart + 0.35);
          });
          break;
        }

        case 'download.done': {
          // Plop
          const osc = this.ctx.createOscillator();
          const gain = this.ctx.createGain();
          osc.type = 'sine';
          osc.frequency.setValueAtTime(320, t);
          osc.frequency.exponentialRampToValueAtTime(640, t + 0.08);
          osc.frequency.exponentialRampToValueAtTime(200, t + 0.16);
          gain.gain.setValueAtTime(0.28, t);
          gain.gain.exponentialRampToValueAtTime(0.001, t + 0.2);
          osc.connect(gain);
          gain.connect(this.masterGain);
          osc.start(t);
          osc.stop(t + 0.2);
          break;
        }

        default:
          break;
      }
    } catch (err) {
      console.warn("Sound play error:", err);
    }
  }
}

export const sounds = new SoundEngine();
