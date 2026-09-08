import { defineConfig } from 'vitest/config';
import { loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import { tanstackRouter } from '@tanstack/router-plugin/vite';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '../..', '');
  const apiUrl = process.env.CATALOG_API_URL ?? env.CATALOG_API_URL;
  const devtoolsEnabled =
    (process.env.CATALOG_DEVTOOLS ?? env.CATALOG_DEVTOOLS) === 'true';

  return {
    define: {
      __CATALOG_DEVTOOLS__: JSON.stringify(devtoolsEnabled),
    },
    plugins: [tanstackRouter({ autoCodeSplitting: true }), react()],
    server: {
      proxy: {
        '/api': {
          target: apiUrl ?? 'http://127.0.0.1:3000',
          changeOrigin: true,
          rewrite: (path) => path.replace(/^\/api/, ''),
        },
      },
    },
    test: {
      clearMocks: true,
      environment: 'node',
      exclude: ['e2e/**', 'node_modules/**'],
      restoreMocks: true,
      setupFiles: ['./src/test/setup.ts'],
    },
  };
});
