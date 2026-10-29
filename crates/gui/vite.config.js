import { defineConfig } from 'vite';

export default defineConfig({
  root: '.',
  server: {
    host: '127.0.0.1',
    port: 5173,
    open: false,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:3001',
        changeOrigin: true,
        // Keep Vite's proxy alive longer than the backend's 120-second AI stream timeout.
        timeout: 130000,
        proxyTimeout: 130000,
        configure: (proxy, _options) => {
          proxy.on('error', (err, _req, res) => {
            if (res && !res.headersSent && typeof res.writeHead === 'function') {
              res.writeHead(503, {
                'Content-Type': 'application/json',
                'Retry-After': '1',
              });
              res.end(JSON.stringify({ error: 'Backend initializing', message: err.message }));
            }
          });
        },
      }
    }
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true
  }
});
