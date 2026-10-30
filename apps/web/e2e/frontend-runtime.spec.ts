import { expect, test, type Page } from '@playwright/test';

const session = {
  user_id: '123e4567-e89b-12d3-a456-426614174000',
  workspace_id: '123e4567-e89b-12d3-a456-426614174001',
  display_name: 'Frontend test',
  email: 'frontend@example.test',
  time_zone: null,
  login_identifier: 'frontend.local',
  capabilities: {
    audit_read: false,
    blueprints_write: true,
    extensions_manage: false,
    extensions_read: false,
    members_manage: false,
    roles_manage: false,
    tokens_manage: false,
    workspace_navigation_manage: false,
  },
};

/** Serves an empty signed-in workspace and blocks every external request. */
const mockApi = (page: Page, baseURL: string, externalScripts: string[] = []) =>
  page.route('**/*', (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== new URL(baseURL).origin) {
      if (route.request().resourceType() === 'script')
        externalScripts.push(url.href);
      // Also block optional external resources such as the app's web fonts.
      return route.abort();
    }
    if (url.pathname.startsWith('/api/')) {
      const body =
        url.pathname === '/api/auth/session'
          ? session
          : url.pathname === '/api/extensions/runtime'
            ? { contributions: [] }
            : [];
      return route.fulfill({ json: body });
    }
    return route.continue();
  });

test('loads Monaco and its worker without external network access', async ({
  page,
  baseURL,
}) => {
  const externalScripts: string[] = [];
  const errors: string[] = [];
  const workers: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('worker', (worker) => workers.push(worker.url()));
  await mockApi(page, baseURL!, externalScripts);
  await page.goto('/manage/blueprints/new');
  await page.getByRole('button', { name: 'Dismiss', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  const editor = page.locator('.monaco-editor').first();
  await expect(editor).toBeVisible();
  const input = editor.getByRole('textbox', { name: 'Editor content' });
  await input.focus();
  await expect(input).toBeFocused();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText(
    'format_version: 1\ncode: offline\nname: Offline\nkind: record\n',
  );
  expect(errors).toEqual([]);
  await expect(editor).toContainText('offline');
  // Editor suggestions exercise the locally configured language features.
  await page.keyboard.press('Control+Space');
  await expect.poll(() => workers.length).toBeGreaterThan(0);
  expect(
    workers.every((url) => new URL(url).origin === new URL(baseURL!).origin),
  ).toBe(true);
  expect(externalScripts).toEqual([]);
  expect(errors).toEqual([]);
});

test('loads Polish translations only after they are selected', async ({
  page,
}) => {
  const localeRequests: string[] = [];
  page.on('request', (request) => {
    if (/\/assets\/(en|pl)-[^/]+\.js$/.test(new URL(request.url()).pathname))
      localeRequests.push(request.url());
  });
  await page.route('**/api/**', (route) =>
    route.fulfill({
      status: 401,
      json: { error: { code: 'unauthorized', message: 'Sign in' } },
    }),
  );
  await page.goto('/login');
  await expect(
    page.getByRole('button', { name: 'Continue', exact: true }),
  ).toBeVisible();
  expect(localeRequests.some((url) => /\/en-/.test(url))).toBe(true);
  expect(localeRequests.some((url) => /\/pl-/.test(url))).toBe(false);
  await page.getByRole('combobox', { name: 'Language' }).click();
  await page.getByRole('option', { name: 'Polish' }).click();
  await expect(page.locator('html')).toHaveAttribute('lang', 'pl');
  expect(localeRequests.some((url) => /\/pl-/.test(url))).toBe(true);
});

test('follows the dark theme and edits YAML with language features', async ({
  page,
  baseURL,
}) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await mockApi(page, baseURL!);
  await page.goto('/manage/blueprints/new');
  await page.getByRole('button', { name: 'Dismiss', exact: true }).click();
  const editor = page.locator('.monaco-editor').first();
  await expect(editor).toBeVisible();
  // The dark design token for surfaces.
  await expect(editor.locator('.monaco-editor-background')).toHaveCSS(
    'background-color',
    'rgb(19, 29, 49)',
  );

  const input = editor.getByRole('textbox', { name: 'Editor content' });
  await input.focus();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText(
    'format_version: 1\ncode: offline\nnme: Offline\nkind: record  # inline note\ntags: [a]\n',
  );
  // Inline comments are highlighted, and flow sequence items read as values
  // like other plain scalars.
  const tokenClass = (text: RegExp) =>
    editor
      .locator('.view-line span span')
      .filter({ hasText: text })
      .first()
      .getAttribute('class');
  await expect.poll(() => tokenClass(/^#.inline.note$/)).not.toBe('mtk1');
  await expect.poll(() => tokenClass(/^a$/)).toBe(await tokenClass(/^record$/));

  // Ctrl+/ toggles a # line comment.
  await page.keyboard.press('ControlOrMeta+Home');
  await page.keyboard.press('ControlOrMeta+/');
  await expect(editor.locator('.view-line').first()).toContainText(
    '# format_version',
  );
  await page.keyboard.press('ControlOrMeta+/');

  // The misspelled key offers its fix.
  await expect(editor.locator('.squiggly-error').first()).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Control+.');
  await expect(page.getByText('Change to “name”')).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(editor).toContainText('name: Offline');
  await expect(editor).not.toContainText('nme');
});
