import {
  createHash,
  createSign,
  generateKeyPairSync,
  randomUUID,
} from 'node:crypto';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { expect, test } from '@playwright/test';

const clientId = 'attricat-e2e';
const clientSecret = 'e2e-client-secret';
const keyId = 'e2e-key';
const signInPath = '/manage/workspace/sign-in';

const base64url = (value: string | Buffer) =>
  Buffer.from(value).toString('base64url');

/** A small real OpenID Connect provider that signs in `subject` at once. */
const startProvider = async () => {
  const { privateKey, publicKey } = generateKeyPairSync('rsa', {
    modulusLength: 2048,
  });
  const codes = new Map<string, { challenge: string; nonce: string }>();
  const login = { subject: 'e2e-owner', email: 'someone@corp.example' };
  let issuer = '';
  const idToken = (nonce: string) => {
    const now = Math.floor(Date.now() / 1000);
    const content = `${base64url(
      JSON.stringify({ alg: 'RS256', kid: keyId, typ: 'JWT' }),
    )}.${base64url(
      JSON.stringify({
        iss: issuer,
        aud: clientId,
        sub: login.subject,
        email: login.email,
        email_verified: true,
        nonce,
        iat: now,
        exp: now + 300,
      }),
    )}`;
    const signature = createSign('RSA-SHA256').update(content).sign(privateKey);
    return `${content}.${base64url(signature)}`;
  };
  const server: Server = createServer((request, response) => {
    const url = new URL(request.url ?? '/', issuer);
    const json = (body: unknown, status = 200) => {
      response.writeHead(status, { 'content-type': 'application/json' });
      response.end(JSON.stringify(body));
    };
    if (url.pathname === '/.well-known/openid-configuration')
      return json({
        issuer,
        authorization_endpoint: `${issuer}/authorize`,
        token_endpoint: `${issuer}/token`,
        jwks_uri: `${issuer}/jwks`,
        response_types_supported: ['code'],
        subject_types_supported: ['public'],
        id_token_signing_alg_values_supported: ['RS256'],
      });
    if (url.pathname === '/jwks')
      return json({
        keys: [
          {
            ...publicKey.export({ format: 'jwk' }),
            kid: keyId,
            use: 'sig',
            alg: 'RS256',
          },
        ],
      });
    if (url.pathname === '/authorize') {
      const code = randomUUID();
      codes.set(code, {
        challenge: url.searchParams.get('code_challenge') ?? '',
        nonce: url.searchParams.get('nonce') ?? '',
      });
      const target = new URL(url.searchParams.get('redirect_uri') ?? '');
      target.searchParams.set('code', code);
      target.searchParams.set('state', url.searchParams.get('state') ?? '');
      response.writeHead(302, { location: target.toString() });
      return response.end();
    }
    if (url.pathname === '/token') {
      let body = '';
      request.on('data', (chunk) => (body += chunk));
      request.on('end', () => {
        const form = new URLSearchParams(body);
        const pending = codes.get(form.get('code') ?? '');
        const challenge = createHash('sha256')
          .update(form.get('code_verifier') ?? '')
          .digest('base64url');
        if (!pending || pending.challenge !== challenge)
          return json({ error: 'invalid_grant' }, 400);
        json({
          access_token: 'e2e-access-token',
          token_type: 'Bearer',
          expires_in: 300,
          id_token: idToken(pending.nonce),
        });
      });
      return;
    }
    response.writeHead(404);
    response.end();
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  issuer = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  return { issuer, login, stop: () => server.close() };
};

// One journey through several browser contexts and first page loads.
test.setTimeout(180_000);

test('sets up single sign-on, signs in with it, and explains a refusal', async ({
  browser,
  page,
}) => {
  const provider = await startProvider();
  try {
    await page.goto(signInPath);
    await page.getByLabel('Button name').fill('E2E SSO');
    await page.getByLabel('Issuer URL').fill(provider.issuer);
    await page.getByLabel('Client ID').fill(clientId);
    await page.getByLabel('Client secret').fill(clientSecret);
    await page.getByRole('button', { name: 'Save provider' }).click();
    await expect(page.getByText('/api/auth/oidc/callback')).toBeVisible();

    // A provider cannot be offered until a test sign-in proves it works.
    const offer = page.getByLabel('Offer this provider on the sign-in page');
    await expect(offer).toBeDisabled();
    await page.getByRole('button', { name: 'Run test sign-in' }).click();
    await expect(page).toHaveURL(new RegExp(`${signInPath}$`));
    await expect(page.getByText('passed a test sign-in')).toBeVisible();
    await offer.click();
    await expect(offer).toBeChecked();

    // A new browser signs in through the provider, with no password.
    const context = await browser.newContext({
      storageState: { cookies: [], origins: [] },
    });
    const visitor = await context.newPage();
    await visitor.goto('/login/default.local');
    await visitor
      .getByRole('button', { name: 'Continue with E2E SSO' })
      .click();
    await expect(
      visitor.getByRole('button', { name: 'Sign out' }),
    ).toBeVisible();
    await context.close();

    // An identity nobody linked gets an explanation, not a session.
    provider.login.subject = 'never-linked';
    const stranger = await browser.newContext({
      storageState: { cookies: [], origins: [] },
    });
    const strangerPage = await stranger.newPage();
    await strangerPage.goto('/login/default.local');
    await strangerPage
      .getByRole('button', { name: 'Continue with E2E SSO' })
      .click();
    await expect(strangerPage.getByRole('alert')).toContainText(
      'no access to this workspace',
    );
    await strangerPage.getByRole('link', { name: 'Back to sign-in' }).click();
    await expect(strangerPage).toHaveURL(/\/login$/);
    await stranger.close();
  } finally {
    await page.goto(signInPath);
    await page.getByRole('button', { name: 'Remove provider' }).first().click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Remove provider' })
      .click();
    await expect(page.getByLabel('Client secret')).toBeVisible();
    provider.stop();
  }
});
