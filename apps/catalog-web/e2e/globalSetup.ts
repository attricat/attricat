import { PostgreSqlContainer } from '@testcontainers/postgresql';
import { chromium, type BrowserContext } from '@playwright/test';
import { execFileSync, spawn, type ChildProcess } from 'node:child_process';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  e2eApiPort,
  e2eApiUrl,
  e2eMailpitSmtpPort,
  e2eMailpitUiPort,
  e2eMailpitUrl,
  e2eS3Port,
  e2eS3Url,
  e2eWebPort,
  e2eWebUrl,
} from './ports.ts';

const workspaceRoot = new URL('../../..', import.meta.url).pathname;
const bootstrapOwnerId = '00000000-0000-4000-8000-000000000201';
const bootstrapOwnerEmail = 'owner@example.test';
const bootstrapOwnerPassword = 'e2e-only-owner-password';
const fixtureEmail = 'fixture@example.test';
const fixturePassword = 'e2e-only-fixture-password';
const resetEmail = 'reset@example.test';
const resetPassword = 'e2e-only-reset-password';
const storageStatePath = new URL('.auth.json', import.meta.url).pathname;

const exampleExtensionId = 'attricat-extension-example';

/** True when `dir` is a checkout of the formula example extension. */
const isExampleExtension = (dir: string) => {
  try {
    const manifest = JSON.parse(
      readFileSync(resolve(dir, 'manifest.json'), 'utf8'),
    );
    return manifest?.catalog?.id === exampleExtensionId;
  } catch {
    return false;
  }
};

// The sibling checkout is two levels above the main checkout and three above
// a worktree under `tasks/`; other repositories may share the directory name.
const exampleExtensionRoot = () => {
  if (process.env.ATTRICAT_EXTENSION_EXAMPLE_DIR)
    return resolve(process.env.ATTRICAT_EXTENSION_EXAMPLE_DIR);
  const candidates = ['../..', '../../..'].map((up) =>
    resolve(workspaceRoot, up, exampleExtensionId),
  );
  const root = candidates.find(isExampleExtension);
  if (!root)
    throw new Error(
      `No ${exampleExtensionId} checkout found at ${candidates.join(' or ')}; set ATTRICAT_EXTENSION_EXAMPLE_DIR`,
    );
  return root;
};

const packageExampleExtension = () => {
  const exampleRoot = exampleExtensionRoot();
  execFileSync('just', ['pack'], { cwd: exampleRoot, stdio: 'inherit' });
  // `just pack` also builds other reference extensions into the same folder.
  const archive = readdirSync(resolve(exampleRoot, 'dist'))
    .filter(
      (name) =>
        name.startsWith(`${exampleExtensionId}-`) && name.endsWith('.tar.zst'),
    )
    .map((name) => ({
      path: resolve(exampleRoot, 'dist', name),
      modified: statSync(resolve(exampleRoot, 'dist', name)).mtimeMs,
    }))
    .sort((left, right) => right.modified - left.modified)[0]?.path;
  if (!archive)
    throw new Error('Example extension packaging produced no archive');
  process.env.CATALOG_E2E_EXAMPLE_EXTENSION_ARCHIVE = archive;
};

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

const waitForMessage = async (email: string) => {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const messages = await fetch(`${e2eMailpitUrl}/api/v1/messages`).then(
      (response) => response.json(),
    );
    const message = messages.messages.find(
      (candidate: { To: Array<{ Address: string }> }) =>
        candidate.To.some((recipient) => recipient.Address === email),
    );
    if (message) {
      return fetch(`${e2eMailpitUrl}/api/v1/message/${message.ID}`).then(
        (response) => response.json(),
      );
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Timed out waiting for onboarding email to ${email}`);
};

const ownerRequest = async (
  context: BrowserContext,
  path: string,
  init: RequestInit = {},
) => {
  const cookies = await context.cookies(e2eWebUrl);
  const csrf = cookies.find((cookie) => cookie.name === 'catalog_csrf');
  const headers = new Headers(init.headers);
  headers.set(
    'Cookie',
    cookies.map((cookie) => `${cookie.name}=${cookie.value}`).join('; '),
  );
  if (
    !['GET', 'HEAD', 'OPTIONS'].includes((init.method ?? 'GET').toUpperCase())
  ) {
    if (!csrf) throw new Error('Owner session did not include a CSRF cookie');
    headers.set('X-Catalog-Csrf', csrf.value);
  }
  const response = await fetch(`${e2eApiUrl}${path}`, { ...init, headers });
  if (!response.ok) {
    throw new Error(
      `E2E owner request failed: ${response.status} ${await response.text()}`,
    );
  }
  return response;
};

const provisionUser = async (
  context: BrowserContext,
  email: string,
  password: string,
) => {
  const session = await ownerRequest(context, '/auth/session').then(
    (response) => response.json(),
  );
  const roles = await ownerRequest(context, '/workspace/assignable-roles').then(
    (response) => response.json(),
  );
  const ownerRole = roles.find(
    (role: { code: string }) => role.code === 'owner',
  );
  if (!ownerRole)
    throw new Error('E2E workspace did not expose the owner role');

  await ownerRequest(context, '/workspace/users', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      email,
      role_id: ownerRole.id,
      scope_type: 'workspace',
      scope_target_id: session.workspace_id,
      expires_at: new Date(Date.now() + 60 * 60 * 1000).toISOString(),
    }),
  });
  const delivered = await waitForMessage(email);
  const rawUrl = JSON.stringify(delivered).match(
    /http:\/\/127\.0\.0\.1:\d+\/onboarding\?[^"\\\s]+/,
  )?.[0];
  if (!rawUrl) throw new Error(`Onboarding email for ${email} had no URL`);
  const url = new URL(
    rawUrl.replaceAll('\\u0026', '&').replaceAll('&amp;', '&'),
  );
  const response = await fetch(`${e2eApiUrl}/onboarding/complete`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      invitation_secret: url.searchParams.get('invitation_secret'),
      onboarding_secret: url.searchParams.get('onboarding_secret'),
      password,
    }),
  });
  if (!response.ok) {
    throw new Error(
      `E2E onboarding failed: ${response.status} ${await response.text()}`,
    );
  }
};

const stop = (process: ChildProcess) => {
  if (!process.killed) process.kill('SIGTERM');
};

export default async () => {
  packageExampleExtension();
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
  const rustfsName = `catalog-e2e-rustfs-${process.pid}`;
  const rustfs = start(
    'docker',
    [
      'run',
      '--rm',
      '--name',
      rustfsName,
      '-p',
      `127.0.0.1:${e2eS3Port}:9000`,
      '-e',
      'RUSTFS_ACCESS_KEY=catalog-e2e',
      '-e',
      'RUSTFS_SECRET_KEY=catalog-e2e-secret',
      'rustfs/rustfs:1.0.0-beta.12',
      '/data',
    ],
    process.env,
  );
  const mailpit = start(
    'docker',
    [
      'run',
      '--rm',
      '-p',
      `127.0.0.1:${e2eMailpitSmtpPort}:1025`,
      '-p',
      `127.0.0.1:${e2eMailpitUiPort}:8025`,
      'axllent/mailpit:v1.28',
    ],
    process.env,
  );
  await waitFor(`${e2eS3Url}/health/live`);
  execFileSync(
    'docker',
    [
      'run',
      '--rm',
      '--network',
      `container:${rustfsName}`,
      '-e',
      'AWS_ACCESS_KEY_ID=catalog-e2e',
      '-e',
      'AWS_SECRET_ACCESS_KEY=catalog-e2e-secret',
      '-e',
      'AWS_DEFAULT_REGION=us-east-1',
      'amazon/aws-cli:2.31.0',
      '--endpoint-url',
      'http://127.0.0.1:9000',
      's3api',
      'create-bucket',
      '--bucket',
      'catalog-files',
    ],
    { stdio: 'inherit' },
  );
  const apiEnv = {
    ...process.env,
    BIND_ADDR: `127.0.0.1:${e2eApiPort}`,
    S3_ENDPOINT: e2eS3Url,
    S3_REGION: 'us-east-1',
    S3_BUCKET: 'catalog-files',
    S3_ACCESS_KEY_ID: 'catalog-e2e',
    S3_SECRET_ACCESS_KEY: 'catalog-e2e-secret',
    S3_FORCE_PATH_STYLE: 'true',
    S3_UPLOAD_TIMEOUT_SECONDS: '30',
    // A cold WASM artifact fetch can exceed the production timeout on Colima.
    S3_DOWNLOAD_TIMEOUT_SECONDS: '120',
    DATABASE_URL: database.getConnectionUri(),
    CATALOG_BOOTSTRAP_OWNER_ID: bootstrapOwnerId,
    CATALOG_BOOTSTRAP_OWNER_EMAIL: bootstrapOwnerEmail,
    CATALOG_BOOTSTRAP_OWNER_PASSWORD: bootstrapOwnerPassword,
    // Specs assume the bootstrap owner is the only seeded member.
    CATALOG_SAMPLE_ACCOUNTS: 'false',
    CATALOG_DEMO_MODE: 'false',
    SESSION_COOKIE_SECURE: 'false',
    SMTP_HOST: '127.0.0.1',
    SMTP_PORT: e2eMailpitSmtpPort,
    SMTP_TLS_MODE: 'disabled',
    HTTP_REQUEST_TIMEOUT_SECONDS: '120',
    PASSWORD_RESET_URL: `${e2eWebUrl}/password-reset/confirm`,
    WORKSPACE_ONBOARDING_URL: `${e2eWebUrl}/onboarding`,
  };
  const api = start('cargo', ['run', '-p', 'api', '--bin', 'api'], apiEnv);
  let fileWorker: ChildProcess | undefined;

  try {
    await waitFor(`${e2eMailpitUrl}/api/v1/messages`);
    await waitFor(`${e2eApiUrl}/health`);
    fileWorker = start('cargo', ['run', '-p', 'api', '--bin', 'file-worker'], {
      ...apiEnv,
      FILE_WORKER_OPERATIONS_BIND_ADDR: '127.0.0.1:0',
    });
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
    await page.getByLabel('Workspace').fill('default.local');
    await page.getByRole('button', { name: 'Continue' }).click();
    await page.waitForURL(`${e2eWebUrl}/login/default.local`);
    await page.getByLabel('Email').fill(bootstrapOwnerEmail);
    await page.getByLabel('Password').fill(bootstrapOwnerPassword);
    await page.getByRole('button', { name: 'Sign in' }).click();
    await page.waitForURL(`${e2eWebUrl}/`);
    await provisionUser(context, fixtureEmail, fixturePassword);
    await provisionUser(context, resetEmail, resetPassword);
    await context.storageState({ path: storageStatePath });
    await browser.close();

    return async () => {
      stop(web);
      if (fileWorker) stop(fileWorker);
      stop(api);
      stop(mailpit);
      stop(rustfs);
      await database.stop();
    };
  } catch (error) {
    if (fileWorker) stop(fileWorker);
    stop(api);
    stop(mailpit);
    stop(rustfs);
    await database.stop();
    throw error;
  }
};
