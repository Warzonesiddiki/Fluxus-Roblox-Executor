import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Vite config for the Tauri frontend.
// `clearScreen: false` keeps Tauri's dev server output readable.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // Tauri expects the dev server on localhost
    host: '127.0.0.1',
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    target: 'es2021',
    minify: 'esbuild',
    sourcemap: false,
  },
});
