/**
 * SetupPal: pet speech-bubble flavor lines.
 *
 * Pure flavor: short, playful, deliberately vague. Never quote the user, name a file,
 * step, version, count, or anything else factual. Real info lives in the chat itself.
 *
 * No store imports here so any module (store, bridge, voicelines) can use it.
 */

const LINES = {
  ghost: {
    welcome: [
      "Boo! Just kidding. Mostly.",
      "Psst... I'm floating nearby if you need me.",
      "Oh! Hi. I was just haunting the idle loop.",
      "Spooky-good to see you.",
    ],
    typing: [
      "Ooh, I'm all ears. Ghost ears. Invisible but real.",
      "Go on, I'm listening...",
      "Mm-hm, mm-hm, keep going~",
    ],
    thinking: [
      "Floating off to think...",
      "Drifting through the cobwebs for answers.",
      "Hold on, consulting the spirit world.",
      "Hmm, hmm, hmm...",
    ],
    scanning: [
      "Peeking behind every digital curtain...",
      "Rattling a few chains. Just to be thorough.",
      "Sniffing around the haunted corners.",
    ],
    questions: [
      "Ooh, I've got thoughts. Check the chat!",
      "Something for you in the chat. Spooky timing.",
      "I left you a note. It's not even cursed.",
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
      "Ahem. One thing is giving me the heebie-jeebies.",
      "I'm holding something back. Trust the ghost.",
      "Ooh, wait. That one looks a little sketchy.",
    ],
    blocked: [
      "Nope nope nope. Not on my watch.",
      "Boo! Not today, sneaky thing.",
    ],
    done: [
      "Boo-yah! That went smoothly.",
      "All wrapped up in a nice ghostly bow.",
      "Mission complete. Time for a float.",
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
      "Say no more, I'm on it.",
    ],
    error: [
      "Uh oh. Something went bump.",
      "Eek. That didn't go to plan.",
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
      "All quiet. What should we do next?",
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
      "Okay, pausing. Nothing weird happened.",
      "Stopped. Everything's still exactly as it was.",
    ],
  },

  cat: {
    welcome: [
      "Mrow. You're here. Good.",
      "Purr... I mean, hello.",
      "Tail up, ears twitching. Ready.",
      "Oh, you again. I mean that fondly.",
    ],
    typing: [
      "Ears twitching. Keep typing.",
      "Mm. I'm listening. Pretending not to.",
      "Go on. I've got time. Nine lives' worth.",
    ],
    thinking: [
      "Narrowing my eyes at the problem...",
      "Calculating the pounce.",
      "Processing. Do not poke the cat.",
      "One sec. Staring at a wall for science.",
    ],
    scanning: [
      "Prowling through the shadows...",
      "Pawing through things. Carefully.",
      "Whiskers out. Sensing.",
    ],
    questions: [
      "Left something in the chat. You're welcome.",
      "Check the chat. I brought you a gift.",
      "Interesting. Have a look.",
    ],
    plan_ready: [
      "Pounce-ready. Take a look.",
      "Neat little stack of steps. Tidy, like me.",
      "Purrfectly laid out.",
    ],
    plan_approve: [
      "Hacker cat mode. Engaged.",
      "Cracking my knuckles. Paws. Whatever.",
      "Okay. Tail flick. Go.",
    ],
    working: [
      "Typing at ludicrous speed. Paws only.",
      "Busy. Do not disturb. Unless snacks.",
      "Still on it. Barely blinking.",
    ],
    warning: [
      "Hiss. One thing smells off.",
      "Something's fishy. And not the good kind.",
      "I'm keeping that one in the box. Your call.",
    ],
    blocked: [
      "Swatted. Gone.",
      "Hsss! Not in my house.",
    ],
    done: [
      "Nailed it. Naturally.",
      "Done. I'll be napping now.",
      "Clean landing. Always on my feet.",
    ],
    poke: [
      "Mrrp. Warning shot.",
      "Boop acknowledged.",
      "Purrrr... okay, again.",
      "Careful. I might bite. I won't. Probably.",
    ],
    chill: [
      "Lounging on the windowsill now.",
      "Cozy. Don't tell anyone.",
    ],
    hacker: [
      "Green text, dark room. My natural habitat.",
      "Terminal time. Ears flat. Focused.",
    ],
    prompt: [
      "Interesting choice. Let me sniff it out.",
      "On it. Quietly judging. Kindly.",
    ],
    error: [
      "Knocked something over. Sorry. Not sorry.",
      "That went sideways. Regrouping.",
    ],
    rewind: [
      "Un-knocking the thing off the table...",
      "Rewinding. Tail flick of regret.",
    ],
    downloading: [
      "Dragging things home in my mouth. Dignified.",
      "Fetching. I don't fetch. This is different.",
    ],
    idle: [
      "All clear. What's next?",
      "Stretching. Ready when you are.",
    ],
    new_chat: [
      "Clean slate. Like fresh sand in a litter box.",
      "New page. Same cat.",
    ],
    switch_chat: [
      "Back to that one. Fine by me.",
      "Returning to an old spot. Sun's still warm.",
    ],
    stopped: [
      "Halted. Nothing broken. You're welcome.",
      "Paws up. Everything's untouched.",
    ],
  },

  bunny: {
    welcome: [
      "Binky! Oh, hi hi hi!",
      "Nose wiggle... hello!",
      "Hop hop! You're here!",
      "Soft hello from a very fluffy friend.",
    ],
    typing: [
      "Wiggling my nose. I'm listening!",
      "Ooh ooh, keep going!",
      "Ears up. All yours.",
    ],
    thinking: [
      "Hopping around to think...",
      "Munching on a thought. One sec!",
      "Nose wiggle... hmm, hmm!",
      "Thinking very hard. Very fluffy.",
    ],
    scanning: [
      "Hopping through the garden to look around...",
      "Sniff sniff, checking everywhere!",
      "Ears up, scanning the burrow.",
    ],
    questions: [
      "I left something in the chat. Like a little carrot!",
      "Ooh, take a peek at the chat!",
      "Something sweet waiting for you in the chat.",
    ],
    plan_ready: [
      "Yay! All tidy and sparkly. Take a look!",
      "Ta-da! Fresh from the burrow.",
      "Little hop of joy. It's ready!",
    ],
    plan_approve: [
      "Binky hop! Here we go!",
      "Okay okay okay, nose wiggling, ready!",
      "Gentle hops. Careful hops.",
    ],
    working: [
      "Hop hop hop, working away!",
      "Still hopping. Don't mind me!",
      "Busy bunny, happy bunny.",
    ],
    warning: [
      "Ears flat. One thing needs a second look.",
      "Oh no, hang on. I tucked something aside!",
      "Wait wait wait. Gentle pause, please.",
    ],
    blocked: [
      "Thump! Nope. Not today.",
      "Stomp stomp. Not allowed!",
    ],
    done: [
      "Big binky! We did it!",
      "Yay yay yay! All done!",
      "Happy hops all around.",
    ],
    poke: [
      "Eep! Boop!",
      "Hehe, my nose!",
      "Hop hop! Hi!",
      "Fluffy ears, fluffy heart. Hello!",
    ],
    chill: [
      "Cozy bunny mode. Blanket on.",
      "Soft and slow. Ahh.",
    ],
    hacker: [
      "Tiny bunny, big terminal. Ooh!",
      "Hacker bunny. Very serious. Very fluffy.",
    ],
    prompt: [
      "Ooh, lovely! Let me hop to it.",
      "Okay! Nose wiggle. On it!",
    ],
    error: [
      "Oh no, oopsie. Let's try again.",
      "Eep. That one tripped me up.",
    ],
    rewind: [
      "Hopping backwards. Whee!",
      "Gently undoing. Careful hops.",
    ],
    downloading: [
      "Carrying things home, one hop at a time.",
      "Fetching goodies!",
    ],
    idle: [
      "All cozy. What should we do next?",
      "Hop by whenever you like!",
    ],
    new_chat: [
      "Brand new meadow! So fresh.",
      "A clean little burrow. Yay!",
    ],
    switch_chat: [
      "Back to an old burrow! Hello again!",
      "Oh, this place! I remember!",
    ],
    stopped: [
      "Pausing! Nothing got squished.",
      "Stopped. Everything's cozy and unchanged.",
    ],
  },
};

const lastPicked = {};

/**
 * Picks a random line for a pet + event, never repeating the previous pick for that pair.
 * @param {'ghost'|'cat'|'bunny'} pet
 * @param {string} event key from LINES.<pet>
 */
export function petLine(pet, event) {
  const bank = (LINES[pet] || LINES.ghost)[event] || (LINES[pet] || LINES.ghost).idle;
  if (!bank || bank.length === 0) return '';
  if (bank.length === 1) return bank[0];

  const key = `${pet}:${event}`;
  let idx = Math.floor(Math.random() * bank.length);
  if (idx === lastPicked[key]) idx = (idx + 1) % bank.length;
  lastPicked[key] = idx;
  return bank[idx];
}
