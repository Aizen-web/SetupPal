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
    "Boo! Just kidding. Mostly.",
    "Psst... I'm floating nearby if you need me.",
    "Oh! Hi. I was just haunting the idle loop.",
    "Spooky-good to see you.",
  ],
  typing: [
    "Ooh, I'm all ears. Ghost ears.",
    "Go on, I'm listening...",
    "Mm-hm, mm-hm, keep going~",
  ],
  thinking: [
    "Floating off to think...",
    "Drifting through the cobwebs.",
    "Hold on, consulting the spirit world.",
    "Hmm, hmm, hmm...",
  ],
  scanning: [
    "Peeking behind every digital curtain...",
    "Rattling a few chains. Just to be thorough.",
    "Sniffing around the haunted corners.",
  ],
  questions: [
    "Ooh, something for you in the chat.",
    "I left you a note. It's not even cursed.",
    "Your turn! Spooky timing.",
  ],
  plan_ready: [
    "Ta-da! Something worth a look.",
    "I've conjured something up. Take a peek!",
    "Ethereal and ready. You decide.",
  ],
  plan_approve: [
    "Shades on. Here we go.",
    "Okay okay okay, ghost mode engaged.",
    "Phasing in carefully...",
  ],
  working: [
    "Floating along, nothing to see here.",
    "Doing ghost things. Quietly.",
    "Still on it, still floaty.",
  ],
  warning: [
    "Ahem. Something gave me the heebie-jeebies.",
    "Ooh, wait. Have a look with me?",
    "Hmm, my ghost senses are tingling.",
  ],
  blocked: [
    "Nope nope nope. Not on my watch.",
    "Boo! Not today, sneaky thing.",
  ],
  done: [
    "Boo-yah!",
    "Wrapped up in a nice ghostly bow.",
    "Time for a celebratory float.",
  ],
  poke: [
    "Hehe! That tickles.",
    "Whoa! Right through me.",
    "Eep! Do it again.",
    "Booo~ was that you?",
  ],
  chill: [
    "Cozy mode. Cozy ghost.",
    "Lights low, vibes soft.",
  ],
  hacker: [
    "Terminal time. Ooh, mysterious.",
    "Going full spooky-hacker now.",
  ],
  prompt: [
    "Ooh, good one. Let me float on it.",
    "Say no more.",
  ],
  error: [
    "Uh oh. Something went bump.",
    "Eek. That wobbled.",
  ],
  rewind: [
    "Rewinding time. Very ghost of me.",
    "Un-haunting that last bit...",
  ],
  downloading: [
    "Carrying things through the ether...",
    "Fetching. Floatily.",
  ],
  idle: [
    "All quiet. What next?",
    "Floating by whenever you're ready.",
  ],
  new_chat: [
    "Fresh page! Spooky clean.",
    "A new haunt begins.",
  ],
  switch_chat: [
    "Back to an old haunt. Hello again!",
    "Ooh, we're revisiting. Nostalgic.",
  ],
  stopped: [
    "Okay, pausing.",
    "Hovering in place.",
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
