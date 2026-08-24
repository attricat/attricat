import { PostgreSqlContainer } from '@testcontainers/postgresql';
import { chromium } from '@playwright/test';
import { execFileSync, spawn, type ChildProcess } from 'node:child_process';
import { e2eApiPort, e2eApiUrl, e2eWebPort, e2eWebUrl } from './ports.ts';

const workspaceRoot = new URL('../../..', import.meta.url).pathname;
const bootstrapOwnerId = '00000000-0000-4000-8000-000000000201';
const bootstrapOwnerEmail = 'owner@example.test';
const bootstrapOwnerPassword = 'e2e-only-owner-password';
const fixtureEmail = 'fixture@example.test';
const fixturePassword = 'e2e-only-fixture-password';
const storageStatePath = new URL('.auth.json', import.meta.url).pathname;

const waitFor = async (url: string) => {
  const deadline = Date.now() + 120_000;
  while (Date.now() < deadline) {
    try {
      if ((await fetch(url)).ok) return;
    } catch {
      // The process has not opened its listener yet.
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`Timed out waiting for ${url}`);
};

const start = (command: string, args: string[], env: NodeJS.ProcessEnv) =>
  spawn(command, args, { cwd: workspaceRoot, env, stdio: 'inherit' });

const stop = (process: ChildProcess) => {
  if (!process.killed) process.kill('SIGTERM');
};

export default async () => {
  // Testcontainers reads DOCKER_HOST rather than Docker CLI contexts. Resolve
  // the active context so Colima and other non-default socket locations work.
  if (!process.env.DOCKER_HOST) {
    process.env.DOCKER_HOST = execFileSync(
      'docker',
      ['context', 'inspect', '--format', '{{.Endpoints.docker.Host}}'],
      { encoding: 'utf8' },
    ).trim();
  }
  // Colima cannot bind-mount its socket into Ryuk. This suite owns the
  // container lifecycle and always stops PostgreSQL in global teardown.
  process.env.TESTCONTAINERS_RYUK_DISABLED = 'true';
  const database = await new PostgreSqlContainer('postgres:18-alpine').start();
  const api = start('cargo', ['run', '-p', 'api'], {
    ...process.env,
    BIND_ADDR: `127.0.0.1:${e2eApiPort}`,
    DATABASE_URL: database.getConnectionUri(),
    CATALOG_BOOTSTRAP_OWNER_ID: bootstrapOwnerId,
    CATALOG_BOOTSTRAP_OWNER_EMAIL: bootstrapOwnerEmail,
    CATALOG_BOOTSTRAP_OWNER_PASSWORD: bootstrapOwnerPassword,
    SESSION_COOKIE_SECURE: 'false',
    CATALOG_E2E_FIXTURE_EMAIL: fixtureEmail,
    CATALOG_E2E_FIXTURE_PASSWORD: fixturePassword,
  });

  try {
    await waitFor(`${e2eApiUrl}/health`);
    const web = start(
      'npm',
      [
        'run',
        'dev',
        '--prefix',
        'apps/catalog-web',
        '--',
        '--host',
        '127.0.0.1',
        '--port',
        e2eWebPort,
      ],
      {
        ...process.env,
        CATALOG_API_URL: e2eApiUrl,
      },
    );
    await waitFor(e2eWebUrl);
    const browser = await chromium.launch();
    const context = await browser.newContext();
    const page = await context.newPage();
    await page.goto(`${e2eWebUrl}/login`);
    await page.getByLabel('Email').fill(bootstrapOwnerEmail);
    await page.getByLabel('Password').fill(bootstrapOwnerPassword);
    await page.getByRole('button', { name: 'Sign in' }).click();
    await page.waitForURL(`${e2eWebUrl}/`);
    await context.storageState({ path: storageStatePath });
    await browser.close();

    return async () => {
      stop(web);
      stop(api);
      await database.stop();
    };
  } catch (error) {
    stop(api);
    await database.stop();
    throw error;
  }
};
