import { defineConfig } from 'vite';

// The UI runs inside the Tauri webview and talks to the embedded backend directly (address and
// token come from the `backend_info` command), so there is no dev proxy. `tauri dev` starts this
// server through `beforeDevCommand` and loads http://localhost:5173.
export default defineConfig({
  root: '.',
  clearScreen: false,
  server: {
    host: 'localhost',
    port: 5173,
    strictPort: true,
    open: false,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2021',
  },
});
