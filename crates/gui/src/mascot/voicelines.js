/**
 * SetupPal: Object-Oriented Pet Voiceline & Event Engine
 * 
 * Provides distinct personality classes for each pet with voicelines
 * and animation states triggered by user and system events.
 */

import { store } from '../state/store.js';
import { sounds } from '../audio/sound-engine.js';

export class PetPersonality {
  constructor(name, title, icon) {
    this.name = name;
    this.title = title;
    this.icon = icon;
    
    // OOP Voicelines Map - maps event names directly to voiceline generator properties
    this.voicelines = {
      welcome: () => this.getWelcome(),
      typing: () => this.getTyping(),
      thinking: (p) => this.getThinking(p?.query),
      diagnosing: () => this.getDiagnosing(),
      questions_asked: () => this.getQuestionsAsked(),
      plan_ready: (p) => this.getPlanReady(p?.stepCount),
      plan_approve: () => this.getPlanApprove(),
      step_progress: (p) => this.getStepProgress(p?.desc),
      warning_held: (p) => this.getWarningHeld(p?.title),
      completed: () => this.getCompleted(),
      poke: () => this.getPoke(),
      mode_toggle: (p) => this.getModeToggle(p?.mode),
      pet_change: () => this.getWelcome(),
      prompt_selected: (p) => this.getPromptSelected(p?.prompt)
    };
  }

  // Base Voiceline Properties & Methods
  getWelcome() {
    return `Hi! I'm ${this.name}, your zero-trust companion. Ask me anything in the chat window, and I'll help you set it up safely!`;
  }

  getTyping() {
    return "I'm listening...";
  }

  getThinking(query) {
    return `Looking into "${query}"... checking all safe routes!`;
  }

  getDiagnosing() {
    return "Inspecting your hardware, runtime, and existing game profiles...";
  }

  getQuestionsAsked() {
    return "I found your system details! Pick an option above or type your preference so I can build the exact plan.";
  }

  getPlanReady(stepCount) {
    return `All set! I drafted ${stepCount} verified steps. Look them over before approving.`;
  }

  getPlanApprove() {
    return "Got it! Running safe verified simulation with zero risk to your PC.";
  }

  getStepProgress(desc) {
    return `Checking: ${desc}`;
  }

  getWarningHeld(stepTitle) {
    return `Hold on! ${stepTitle} needs your review before we touch anything.`;
  }

  getCompleted() {
    return "All safe steps completed cleanly! Your system is pristine.";
  }

  getPoke() {
    return `Hehe! ${this.name} is standing by to protect your system!`;
  }

  getModeToggle(mode) {
    return mode === 'hacker'
      ? "Terminal view engaged. Monitoring every sandbox event!"
      : "Chill view engaged. Relax, I'm keeping everything safe.";
  }

  getPromptSelected(prompt) {
    return `Great choice! Looking into "${prompt}" safely!`;
  }
}

export class GhostPersonality extends PetPersonality {
  constructor() {
    super("Pixel", "Pixel the Ghost", "ghost");
  }

  getWelcome() {
    return "Hey there! I'm Setup Pal. I'll help you set up your development tools and environments safely!";
  }

  getTyping() {
    return "Ooh, tell me more! I'm listening closely...";
  }

  getThinking(query) {
    return `Floating through the archives for "${query}"... making sure it's 100% safe!`;
  }

  getDiagnosing() {
    return "Scanning host machine... floating past your Java runtime and Minecraft folders!";
  }

  getQuestionsAsked() {
    return "I diagnosed your PC! Pick an option above so I can prepare a safe, verified plan.";
  }

  getPlanReady(stepCount) {
    return `Ethereal shield ready! Formulated ${stepCount} safe steps with cryptographic checks.`;
  }

  getPlanApprove() {
    return "Putting on my shades! Simulating every step through the zero-trust radar.";
  }

  getWarningHeld(stepTitle) {
    return `Caution! My radar flagged "${stepTitle}". I held it back for your call!`;
  }

  getCompleted() {
    return "Fanfare! All verified steps simulated successfully without altering your computer.";
  }

  getPoke() {
    return "Wheee! You tickled a ghost! Ready to help you set up anything!";
  }

  getPromptSelected(prompt) {
    return `Ooh, "${prompt}"! Ghost radar active, analyzing dependencies safely!`;
  }
}

export class CatPersonality extends PetPersonality {
  constructor() {
    super("Byte", "Byte the Cyber Cat", "cat");
  }

  getWelcome() {
    return "Purrr! I'm Byte the Cyber Cat. Ask me anything in the chat window, and I'll keep your machine safe!";
  }

  getTyping() {
    return "Twitching ears... I hear you typing!";
  }

  getThinking(query) {
    return `Sniffing out "${query}" across trusted repos... No malware gets past these claws!`;
  }

  getDiagnosing() {
    return "Pawing through your system paths... Checking Java and installed game directories!";
  }

  getQuestionsAsked() {
    return "Found your setup! Click an option above or type me an answer, and I'll pounce on the plan.";
  }

  getPlanReady(stepCount) {
    return `Purrrfect! Crafted ${stepCount} verified steps. Every single hash is accounted for!`;
  }

  getPlanApprove() {
    return "Hacker cat mode activated! Running dry-run simulation at lightspeed!";
  }

  getWarningHeld(stepTitle) {
    return `Hiss! "${stepTitle}" is brand new or unverified. I held it in quarantine!`;
  }

  getCompleted() {
    return "Meow! Safe simulation complete! Your PC remains totally clean.";
  }

  getPoke() {
    return "Purrrrr! Byte loves clean verified packages and head scratches!";
  }

  getPromptSelected(prompt) {
    return `Pouncing on "${prompt}"! Let's verify every binary with our zero-trust claws!`;
  }
}

export class BunnyPersonality extends PetPersonality {
  constructor() {
    super("Mochi", "Mochi the Bunny", "bunny");
  }

  getWelcome() {
    return "Binky! I'm Mochi the Bunny. Ask me anything in the chat window and we'll set it up safely!";
  }

  getTyping() {
    return "Wiggling my nose... What are we setting up?";
  }

  getThinking(query) {
    return `Hopping into action for "${query}"! Checking our zero-trust safety checks!`;
  }

  getDiagnosing() {
    return "Hopping through your PC! Checking Java runtimes and Minecraft installations!";
  }

  getQuestionsAsked() {
    return "System check done! Pick a flavor above and I'll hop right to the plan!";
  }

  getPlanReady(stepCount) {
    return `Hooray! ${stepCount} sparkling clean steps ready. Zero random downloads allowed!`;
  }

  getPlanApprove() {
    return "Binky hop! Testing every step in our safe sandbox simulation!";
  }

  getWarningHeld(stepTitle) {
    return `Ears up! "${stepTitle}" needs caution. I tucked it safely away for your review!`;
  }

  getCompleted() {
    return "Yay, big binky! Everything checked out safe and sound!";
  }

  getPoke() {
    return "Hop hop! Mochi is happy to help you build clean setups!";
  }

  getPromptSelected(prompt) {
    return `Binky hop! "${prompt}" is a great goal. Checking hashes before we touch a thing!`;
  }
}

export class PetVoiceEngine {
  constructor() {
    this.personalities = {
      ghost: new GhostPersonality(),
      cat: new CatPersonality(),
      bunny: new BunnyPersonality(),
    };
  }

  getCurrentPersonality() {
    const petKey = store.getState().activePet || 'ghost';
    return this.personalities[petKey] || this.personalities.ghost;
  }

  trigger(event, payload = {}) {
    const pet = this.getCurrentPersonality();
    let text = "";
    let mascotState = "idle";
    let soundName = null;

    switch (event) {
      case 'welcome':
        text = pet.getWelcome();
        mascotState = "idle";
        break;
      case 'typing':
        text = pet.getTyping();
        mascotState = "listening";
        break;
      case 'thinking':
        text = pet.getThinking(payload.query || "your request");
        mascotState = "thinking";
        break;
      case 'diagnosing':
        text = pet.getDiagnosing();
        mascotState = "scanning";
        soundName = "ui.click";
        break;
      case 'questions_asked':
        text = pet.getQuestionsAsked();
        mascotState = "speaking";
        soundName = "msg.receive";
        break;
      case 'plan_ready':
        text = pet.getPlanReady(payload.stepCount || 5);
        mascotState = "idle";
        soundName = "plan.ready";
        break;
      case 'plan_approve':
        text = pet.getPlanApprove();
        mascotState = "speaking";
        soundName = "ui.click";
        break;
      case 'step_progress':
        text = pet.getStepProgress(payload.desc || "Verifying");
        mascotState = "running";
        break;
      case 'warning_held':
        text = pet.getWarningHeld(payload.title || "Step");
        mascotState = "yellow";
        soundName = "verdict.yellow";
        break;
      case 'completed':
        text = pet.getCompleted();
        mascotState = "celebrate";
        soundName = "task.done";
        break;
      case 'poke':
        text = pet.getPoke();
        mascotState = "celebrate";
        soundName = "task.done";
        break;
      case 'mode_toggle':
        text = pet.getModeToggle(payload.mode || 'chill');
        mascotState = payload.mode === 'hacker' ? 'speaking' : 'idle';
        soundName = "ui.click";
        break;
      case 'pet_change':
        text = pet.getWelcome();
        mascotState = "wave-hello";
        soundName = "ui.click";
        break;
      case 'prompt_selected':
        text = pet.getPromptSelected(payload.prompt || "your goal");
        mascotState = "thinking";
        soundName = "ui.click";
        break;
      default:
        text = pet.getWelcome();
        break;
    }

    if (soundName) {
      sounds.play(soundName);
    }
    store.setMascotState(mascotState, text);
  }
}

export const petVoiceEngine = new PetVoiceEngine();
