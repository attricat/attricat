import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { tanstackRouter } from '@tanstack/router-plugin/vite';

const trustedUserId = process.env.CATALOG_TRUSTED_USER_ID;
const trustedWorkspaceId = process.env.CATALOG_TRUSTED_WORKSPACE_ID;
const trustedApiHeaders =
  trustedUserId && trustedWorkspaceId
    ? {
        'X-Catalog-User-Id': trustedUserId,
        'X-Catalog-Workspace-Id': trustedWorkspaceId,
      }
    : undefined;

export default defineConfig({
  plugins: [tanstackRouter(), react()],
  server: {
    proxy: {
      '/api': {
        target: process.env.CATALOG_API_URL ?? 'http://127.0.0.1:3000',
        changeOrigin: true,
        headers: trustedApiHeaders,
        rewrite: (path) => path.replace(/^\/api/, ''),
      },
    },
  },
  test: {
    environment: 'node',
    exclude: ['e2e/**', 'node_modules/**'],
  },
});
