import { createServer } from 'node:net';
import { defineConfig, devices } from '@playwright/test';

// Frontend-only production smoke tests: no API process or database is started.
const port =
  process.env.ATTRICAT_FRONTEND_TEST_PORT ??
  String(
    await new Promise<number>((resolve, reject) => {
      const server = createServer();
      server.once('error', reject);
      server.listen(0, '127.0.0.1', () => {
        const address = server.address();
        if (!address || typeof address === 'string')
          return reject(new Error('No test port'));
        server.close((error) =>
          error ? reject(error) : resolve(address.port),
        );
      });
    }),
  );
// Workers reload this config; inherit the port allocated by the runner.
process.env.ATTRICAT_FRONTEND_TEST_PORT = port;
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: './e2e',
  testMatch: 'frontend-runtime.spec.ts',
  forbidOnly: Boolean(process.env.CI),
  workers: 1,
  use: { ...devices['Desktop Chrome'], baseURL, trace: 'retain-on-failure' },
  webServer: {
    command: `pnpm exec vite preview --host 127.0.0.1 --port ${port} --strictPort`,
    url: baseURL,
    reuseExistingServer: false,
  },
});
