import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

declare const process: { env: Record<string, string | undefined> };

// The Rust server serves dist/ at / with SPA fallback and the API at /api on the same origin.
// In development, /api is proxied to a locally running mp-server.
export default defineConfig(({ command }) => {
  // A stray NODE_ENV in the shell (anything but "production") would otherwise ship React's
  // development build. Builds are always production builds.
  if (command === 'build') process.env.NODE_ENV = 'production';
  return {
    plugins: [react()],
    server: {
      proxy: {
        '/api': {
          target: 'http://localhost:18500',
          changeOrigin: true,
        },
      },
    },
    build: {
      outDir: 'dist',
      emptyOutDir: true,
      sourcemap: false,
    },
  };
});
