/**
 * SetupPal: composer slash commands.
 *
 * Commands run locally and never reach the model. Other modules can add commands with
 * registerSlashCommand (for example a future /stop that cancels a running plan).
 */

const commands = new Map();

/**
 * @param {object} cmd
 * @param {string} cmd.name      lowercase, without the slash
 * @param {string} cmd.hint      one short line for the command menu
 * @param {(ctx: object) => (void|Promise<void>)} cmd.run
 *        ctx = { args, notify(text, tone?) }
 */
export function registerSlashCommand(cmd) {
  commands.set(cmd.name, cmd);
}

export function listSlashCommands() {
  return [...commands.values()];
}

/** Commands whose name starts with what was typed after "/". */
export function matchSlashCommands(text) {
  const m = String(text || '').match(/^\/([a-z]*)$/i);
  if (!m) return [];
  const prefix = m[1].toLowerCase();
  return listSlashCommands().filter(c => c.name.startsWith(prefix));
}

/**
 * Treats "/word" (optionally followed by arguments) as a command.
 * Returns null for normal text, including paths like "/usr/bin" or "/ is cool".
 */
export function parseSlashCommand(text) {
  const m = String(text || '').trim().match(/^\/([a-z]+)(?:\s+(.*))?$/i);
  if (!m) return null;
  return { name: m[1].toLowerCase(), args: (m[2] || '').trim() };
}

/**
 * Runs the command in text. Returns true when the text was a command (known or not),
 * false when it should be sent to the model as a normal message.
 */
export async function runSlashCommand(text, ctx) {
  const parsed = parseSlashCommand(text);
  if (!parsed) return false;
  const cmd = commands.get(parsed.name);
  if (!cmd) {
    if (parsed.args) return false;
    ctx.notify(`Unknown command /${parsed.name}. Type /help to see what works.`, 'warn');
    return true;
  }
  await cmd.run({ ...ctx, args: parsed.args });
  return true;
}
