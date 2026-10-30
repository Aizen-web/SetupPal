/**
 * SetupPal: Pixel's speech-bubble lines.
 *
 * Pure flavor: short, playful, deliberately vague. A line never quotes the user and never
 * names a file, step, version, count, or result. Real information lives in the chat,
 * the plan card, and the terminal.
 *
 * No store imports here so any module (store, bridge, voicelines) can use it.
 */

const LINES = {
  welcome: [
    "Hi! Pixel reporting for tiny-helper duty.",
    "Fresh haunt. Good vibes only.",
    "Ready when you are.",
    "Oh, hi! I saved you the comfy seat.",
  ],
  typing: [
    "Ooh, tell me more.",
    "I'm listening—floatily.",
    "Yep, I've got you.",
  ],
  thinking: [
    "One tiny think coming up...",
    "Let me connect the little dots.",
    "Braincell: activated.",
    "Floating through the possibilities.",
  ],
  scanning: [
    "Peeking into the important corners.",
    "Tiny safety goggles: on.",
    "Giving this a careful once-over.",
  ],
  questions: [
    "Tiny question incoming.",
    "Your turn—I'll hover right here.",
    "One quick detail, then we're golden.",
  ],
  plan_ready: [
    "Ooh, this came together nicely.",
    "Plan-shaped and ready for a peek.",
    "All lined up. You get the final say.",
  ],
  plan_approve: [
    "Tiny paws, big focus. Let's go.",
    "Locked in and floating forward.",
    "Okay—careful mode engaged.",
  ],
  working: [
    "Making the little gears go.",
    "On it. Float, float, float.",
    "Progress looks good on us.",
  ],
  warning: [
    "Tiny pause—this deserves a look.",
    "My careful-sense is tingling.",
    "Let's check this bit together.",
  ],
  blocked: [
    "Nope. Keeping you safe wins this round.",
    "Blocked and tucked safely away.",
    "Not getting past this little ghost.",
  ],
  done: [
    "Boo-yah. Nailed it.",
    "Done and dusted. Tiny victory lap!",
    "Look at us go.",
    "A very satisfying little win.",
  ],
  poke: [
    "Hehe—direct hit!",
    "Boop received. Morale increased.",
    "Eep! My one weakness: friendship.",
    "Achievement unlocked: poked a ghost.",
  ],
  chill: [
    "Cozy mode: restored.",
    "Lights low, vibes immaculate.",
  ],
  hacker: [
    "Terminal time. Tiny shades on.",
    "Serious screen, unserious ghost.",
  ],
  prompt: [
    "Ooh, good one. Let me float on it.",
    "Say less. Thinking now.",
  ],
  error: [
    "Tiny wobble. We'll sort it out.",
    "Oops. That needs another look.",
    "Well, that wasn't the fun kind of surprise.",
  ],
  rewind: [
    "Rolling that back, nice and carefully.",
    "Un-haunting the last little bit.",
  ],
  downloading: [
    "Fetching the bits and bytes.",
    "Carrying things through the ether.",
  ],
  idle: [
    "All quiet. I'm right here.",
    "Floating nearby whenever you're ready.",
    "No rush. The vibe is patient.",
  ],
  new_chat: [
    "Fresh page. Maximum possibility.",
    "New haunt, clean slate.",
  ],
  switch_chat: [
    "Back to this little corner. Hi again!",
    "Oh, I remember this one.",
  ],
  stopped: [
    "Paused. I'll hover right here.",
    "All stopped, all good.",
  ],
};

let lastPicked = {};

/**
 * Picks a random line for an event, never repeating the previous pick for that event.
 * @param {string} event key from LINES
 */
export function pixelLine(event) {
  const bank = LINES[event] || LINES.idle;
  if (bank.length === 1) return bank[0];

  let idx = Math.floor(Math.random() * bank.length);
  if (idx === lastPicked[event]) idx = (idx + 1) % bank.length;
  lastPicked = { ...lastPicked, [event]: idx };
  return bank[idx];
}
