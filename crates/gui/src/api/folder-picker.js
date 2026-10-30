/**
 * Native folder picker (Tauri dialog plugin) for plans that install npm packages.
 * Returns the chosen folder, or null when the user cancels.
 */

import { open } from '@tauri-apps/plugin-dialog';

export async function pickProjectFolder() {
  const chosen = await open({
    directory: true,
    multiple: false,
    title: 'Choose the project folder for npm packages',
  });
  return typeof chosen === 'string' && chosen.trim() ? chosen : null;
}
