import { readFile } from 'node:fs/promises';
import { ANIMATIONS } from '../src/mascot/animations.js';

const [animationCss, lab, settings] = await Promise.all([
  readFile(new URL('../src/styles/ghost.css', import.meta.url), 'utf8'),
  readFile(new URL('../src/components/animation-lab.js', import.meta.url), 'utf8'),
  readFile(new URL('../src/components/settings-view.js', import.meta.url), 'utf8'),
]);

const errors = [];
const ids = new Set();
const keyframes = new Set(
  [...animationCss.matchAll(/@keyframes\s+([\w-]+)/g)].map((match) => match[1]),
);

for (const animation of ANIMATIONS) {
  if (ids.has(animation.id)) errors.push(`Duplicate animation ID: ${animation.id}`);
  ids.add(animation.id);

  const stateSelector = new RegExp(
    `data-state=["']${animation.id.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}["']`,
  );
  if (!stateSelector.test(animationCss)) {
    errors.push(`No CSS state selector found for "${animation.id}"`);
  }

  if (animation.id !== 'idle') {
    const stateRules = animationCss.split('}').filter((rule) => stateSelector.test(rule));
    const names = stateRules.flatMap((rule) =>
      [...rule.matchAll(/\banimation(?:-name)?\s*:\s*([\w-]+)/gi)]
        .map((match) => match[1])
        .filter((name) => name !== 'none'),
    );
    if (names.length === 0) {
      errors.push(`No animation declaration found for "${animation.id}"`);
    } else if (!names.some((name) => keyframes.has(name))) {
      errors.push(`No matching @keyframes rule found for "${animation.id}"`);
    }
  }
}

if (!lab.includes('ANIMATIONS.length')) {
  errors.push('Mascot Lab count is not derived from ANIMATIONS.length');
}

// Settings hosts the Mascot Lab entry point; any count it shows must come from the registry.
if (/\b\d+\s+(?:animations|animation states|states)\b/i.test(settings)) {
  errors.push('Settings shows a hardcoded animation count; derive it from ANIMATIONS.length');
}

if (errors.length > 0) {
  console.error(`Animation registry check failed:\n- ${errors.join('\n- ')}`);
  process.exitCode = 1;
} else {
  console.log(`Animation registry check passed (${ANIMATIONS.length} unique states).`);
}
