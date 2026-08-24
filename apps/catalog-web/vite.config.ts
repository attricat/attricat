import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { tanstackRouter } from '@tanstack/router-plugin/vite';

export default defineConfig({
  plugins: [tanstackRouter(), react()],
  server: {
    proxy: {
      '/api': {
        target: process.env.CATALOG_API_URL ?? 'http://127.0.0.1:3000',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, ''),
      },
    },
  },
  test: {
    environment: 'node',
    exclude: ['e2e/**', 'node_modules/**'],
  },
});
