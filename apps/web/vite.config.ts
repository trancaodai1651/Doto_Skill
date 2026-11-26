/// <reference types="vitest/config" />
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

const backend = process.env.DOTO_API_URL ?? 'http://127.0.0.1:4319';

export default defineConfig({
  base: '/',
  plugins: [react(), tailwindcss()],
  server: {
    port: 5173,
    // Keys starting with ^ are regexes. Plain '/s' would be a prefix match and
    // swallow SPA routes like /skills, /server, /settings.
    proxy: {
      '^/api/': { target: backend, changeOrigin: false },
      '^/s/': { target: backend, changeOrigin: false },
    },
  },
  preview: {
    port: 4173,
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false,
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
  },
});
