import { type Browser, expect, type Page, test } from '@playwright/test';
import { e2eApiUrl } from './ports.ts';
import { e2eLoginIdentifier } from './workspace-identity.ts';

// The suite's API runs with one configuration, so the single-workspace and
// default-language cases replace the client config this server publishes.
// The API tests cover what the server publishes for each setting.

const signedOut = { storageState: { cookies: [], origins: [] } };

const withClientConfig = async (
  browser: Browser,
  config: {
    default_language: 'en' | 'pl' | null;
    sign_in_workspace: string | null;
  },
) => {
  const context = await browser.newContext(signedOut);
  await context.route('**/api/client-config', (route) =>
    route.fulfill({ json: config }),
  );
  return { context, page: await context.newPage() };
};

const signInAsFixture = async (page: Page) => {
  await page.getByLabel('Email').fill('fixture@example.test');
  await page.getByLabel('Password').fill('e2e-only-fixture-password');
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible();
};

test('the server publishes the defaults it was started with', async () => {
  const response = await fetch(`${e2eApiUrl}/client-config`);
  expect(response.ok).toBe(true);
  expect(await response.json()).toEqual({
    default_language: null,
    sign_in_workspace: null,
  });
});

test('the workspace with a configured identifier has its default context', async ({
  page,
}) => {
  const session = await page.request.get('/api/auth/session');
  expect((await session.json()).login_identifier).toBe(e2eLoginIdentifier);
  const contexts = await page.request.get('/api/contexts');
  expect(contexts.ok()).toBe(true);
  const codes = ((await contexts.json()) as { code: string }[]).map(
    ({ code }) => code,
  );
  expect(codes).toContain('default');
});

test('the app names the workspace over the logo and in the Explorer', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('img', { name: 'Attricat' }).hover();
  await expect(page.getByRole('tooltip')).toHaveText('Default workspace');
  // The Explorer pane is open on the home route.
  await expect(
    page
      .getByRole('navigation', { name: 'Record explorer' })
      .getByRole('heading', { name: 'Default workspace' }),
  ).toBeVisible();
});

test('sign-in skips the workspace step in a single-workspace deployment', async ({
  browser,
}) => {
  const { context, page } = await withClientConfig(browser, {
    default_language: null,
    sign_in_workspace: e2eLoginIdentifier,
  });
  await page.goto('/');
  await expect(page).toHaveURL(
    (url) => url.pathname === `/login/${e2eLoginIdentifier}`,
  );
  await expect(page.getByLabel('Workspace', { exact: true })).toHaveCount(0);
  await expect(
    page.getByRole('heading', { name: 'Sign in to Default workspace' }),
  ).toBeVisible();
  await expect(page.getByText(e2eLoginIdentifier)).toBeVisible();
  await expect(
    page.getByRole('link', { name: 'Change workspace' }),
  ).toHaveCount(0);
  await signInAsFixture(page);

  // Signing out lands on the same step, not on a workspace prompt.
  await page.getByRole('button', { name: 'Sign out' }).click();
  await expect(page).toHaveURL(
    (url) => url.pathname === `/login/${e2eLoginIdentifier}`,
  );
  await context.close();
});

test('the installation language applies until someone picks another', async ({
  browser,
}) => {
  const { context, page } = await withClientConfig(browser, {
    default_language: 'pl',
    sign_in_workspace: null,
  });
  await page.goto('/login');
  await expect(
    page.getByRole('heading', { name: 'Zaloguj się' }),
  ).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'pl');

  await page.getByRole('combobox', { name: 'Język' }).click();
  await page.getByRole('option', { name: 'Angielski' }).click();
  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible();
  await context.close();
});
