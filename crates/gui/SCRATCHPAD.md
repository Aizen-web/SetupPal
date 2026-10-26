# SetupPal UI Implementation Scratchpad

## 🎯 Executive Summary & Mission
SetupPal's GUI has been built from the ground up according to [ui.md](file:///e:/Vs%20Code/SetupPal/ui.md), [TODO.md](file:///e:/Vs%20Code/SetupPal/TODO.md), and your design mockup.
The backend logic in `crates/core` and `crates/cli` was left completely untouched. The UI runs directly as a **native desktop application window** (1100 × 720 px) on your laptop.

---

## 🐾 The 3 Kawaii Pets & Their Personalities

You can now toggle seamlessly between **3 Kawaii Mascot Pets** using the top pill bar (`[ 👻 Pixel | 🐱 Byte | 🐰 Mochi ]`), the Settings view, or the Mascot Lab:

### 1. 👻 Pixel the Ghost (Ethereal Zero-Trust Guardian)
- **Personality**: Gentle, shy, ethereal, thorough.
- **Visuals**: Soft lavender gradient body (`#F4EFFF` to `#E2D7FF`), lavender outline, cute antennae, rippling hem, rosy cheeks (`#F8BBD9`).
- **Behaviors**:
  - *Normal*: Floats smoothly with an ambient shadow that expands and contracts.
  - *Speaking*: Gentle mouth squashing in sync with the speech bubble.
  - *Errors & Red Threats*: **No longer smiles!** Displays sad downturned brows (`brows-sad`), teary glistening anime eyes (`eyes-sad`), a sad pout (`mouth-sad`), and **animated streaming teardrops** pouring down with tear puddles (`#fx-tears`).

### 2. 🐱 Byte the Cat (Cyber Hacker Feline)
- **Personality**: Feisty, hyper-focused, tech-savvy hacker cat.
- **Visuals**: Cream-caramel body (`#FFE8D6` to `#F2CFA5`), inner pink ears, twitching cat tail, cheek blush with whiskers, and a glowing green cyber collar with neon charm.
- **Behaviors**:
  - *Idle*: Tail swishes back and forth; left ear twitches periodically.
  - *Working*: Tiny paws type furiously on an imaginary keyboard ("tippy-tappy" typing).
  - *Downloading*: Carries a package box marked with a neon fish icon.
  - *Glasses*: Sleek cat-eye shaped neon green hacker shades.
  - *Errors & Red Threats*: Sad kitten pout, ears droop down flat, and animated kitten tears stream down!
  - *Celebrate*: Playful cat pounce with floating sparkles.

### 3. 🐰 Mochi the Bunny (Cozy Mochi Rabbit)
- **Personality**: Soft, sweet, cautious, snacker, joyful bounciness.
- **Visuals**: Round marshmallow mochi body (`#FFFFFF` to `#FCEEE6`), long floppy bunny ears, little pink heart nose with twitching whiskers, round cotton tail.
- **Behaviors**:
  - *Idle*: Floppy ears bounce gently in rhythm with its float.
  - *Working*: Construction hardhat with custom ear cutouts, eager rapid hops.
  - *Downloading*: Carries a box marked with a mini carrot badge.
  - *Glasses*: Round retro-futuristic cyber bunny goggles.
  - *Green / Celebrate*: Performs a joyful high **binky jump** (happy twist in midair)!
  - *Errors & Red Threats*: Floppy ears droop to the ground, sad quivering mouth, and fountain tears.

---

## 🎬 20 Custom SVG Animations

Every single pet supports the complete 20-animation catalogue with pet-specific flair:

| # | ID | Visual Action |
|---|---|---|
| 1 | `idle-float` | Vertical levitation, inverse ground shadow scaling, tail swish & ear bounce |
| 2 | `blink` | Realistic 120ms eyelid scaleY blink |
| 3 | `speaking` | Mouth squashing in sync with speech typewriter text |
| 4 | `thinking` | Head tilted 4° with 3 staggered pulsing thought dots |
| 5 | `scanning` | Translucent green flashlight beam sweeping with eye tracking |
| 6 | `working` | Hardhat equipped; rapid float; sparkles; cat paws type |
| 7 | `glasses-on` | Spring-loaded sunglasses slide down with animated lens glint sweep |
| 8 | `glasses-off` | Sunglasses slide away upwards |
| 9 | `green` | Happy arc eyes, sparkle burst, and joyful hop (or bunny binky) |
| 10 | `yellow` | Head tilt 8°, worried brow, and falling sweat drop |
| 11 | `red` | **Sad crying face, streaming tears**, defensive shield pop-in, warning wobble |
| 12 | `celebrate` | 360° victory spin, confetti shower, and victory pose |
| 13 | `wave-hello` | Playful left & right welcoming wave with waving right paw |
| 14 | `listening` | **Leans forward toward user**, eyes widen, ears perk when typing in composer |
| 15 | `downloading` | Carries package box, speed ripple hover, rapid hem ripples |
| 16 | `error` | **Sad downturned mouth, sad drooping brows, streaming animated tears** |
| 17 | `held-sign` | Holds up yellow caution warning sign ("HELD", "PAWS!", "WAIT!") |
| 18 | `rewind` | Counter-clockwise rewind spin with backwards spinning clock hands |
| 19 | `sleepy` | Half-closed eyes with floating animated Zzz bubbles |
| 20 | `peek-a-boo` | Slides in from edge, peeks shyly, then settles into center |

---

## 🎨 Dual Mode System (Chill & Hacker)

### 1. Chill Mode (Default - Warm Cream, Beige, Caramel)
- **Palette**: `#F5EEDF` (Cream), `#FBF6EA` (Surface), `#EFE6D3` (Sand), `#3B2F2A` (Cocoa text).
- **Plan Card**: "Minecraft with performance mods" (5 steps, 4 safe, 1 held) with interactive approve and review buttons.
- **TASKS Panel**: Progress tracker (`2 of 4 done`, `1 held · 0 blocked`).

### 2. Hacker Mode (Auto-Enters on Execution & Downloading)
- **Palette**: `#06130A` (Deep Cyber Black), `#0B2A12` (Terminal Green), `#39FF14` (Electric Neon).
- **Auto-Transition**: Activates immediately upon task execution or file download.
- **Glasses**: Neon sunglasses snap onto the active pet with lens glint.
- **Terminal**: Live stream of verification logs (`> plan approved`, `✓ sha256 matches publisher`, `~ downloading...`).
- **Auto-Return**: Celebrates with confetti upon completion and returns to Chill Mode after dwell time.

---

## 🔊 Cozy 8-Bit Web Audio Engine
Integrated synthesizer in [sound-engine.js](file:///e:/Vs%20Code/SetupPal/crates/gui/src/audio/sound-engine.js) providing responsive retro sound effects:
- `ui.click`, `msg.send`, `msg.receive`, `mode.hacker` (synth sweep up), `mode.chill` (warm sweep down), `task.start`, `download.done`, `verdict.green`, `verdict.yellow`, `verdict.red` (buzz), and `celebrate` (victory arpeggio).

---

## 🚀 How to Launch on Your Laptop
- Run [start-app.bat](file:///e:/Vs%20Code/SetupPal/start-app.bat) in the project root.
- Or in terminal:
  ```powershell
  cd "e:\Vs Code\SetupPal\crates\gui"
  npm run app
  ```
