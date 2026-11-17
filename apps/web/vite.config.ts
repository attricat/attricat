import { defineConfig } from 'vitest/config';
import { loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import { tanstackRouter } from '@tanstack/router-plugin/vite';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '../..', '');
  const apiUrl = process.env.ATTRICAT_API_URL ?? env.ATTRICAT_API_URL;
  const devtoolsEnabled =
    mode === 'test' ||
    (process.env.ATTRICAT_DEVTOOLS ?? env.ATTRICAT_DEVTOOLS) === 'true';

  return {
    define: {
      __ATTRICAT_DEVTOOLS__: JSON.stringify(devtoolsEnabled),
    },
    plugins: [tanstackRouter({ autoCodeSplitting: true }), react()],
    build: {
      rolldownOptions: {
        output: {
          // Keep the modules the entry loads at start-up in the entry chunk, as
          // Rollup did; Rolldown otherwise splits them into many small chunks
          // that the first page load fetches one by one.
          codeSplitting: { groups: [{ name: 'index', tags: ['$initial'] }] },
        },
      },
    },
    server: {
      proxy: {
        // The API serves its routes below `/api`, so requests pass through
        // unchanged to the origin of its base URL.
        '/api': {
          target: new URL(apiUrl ?? 'http://127.0.0.1:3000/api').origin,
          changeOrigin: true,
        },
      },
    },
    test: {
      clearMocks: true,
      environment: 'node',
      exclude: ['e2e/**', 'node_modules/**'],
      // Each test starts from the mocks' original implementations:
      // `restoreMocks` only restores `vi.spyOn` spies since Vitest 4.
      mockReset: true,
      restoreMocks: true,
      setupFiles: ['./src/test/setup.ts'],
    },
  };
});
