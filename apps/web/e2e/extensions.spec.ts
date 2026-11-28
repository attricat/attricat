import { expect, test } from '@playwright/test';
import {
  createRecord,
  createRecordBlueprint,
  defaultContext,
  request,
  scalar,
  suffix,
} from './helpers';

test('side-loads the example extension and recalculates a blueprint formula', async ({
  page,
}) => {
  // The first invocation fetches and compiles the packaged WASM on a cold host.
  test.setTimeout(180_000);
  const archive = process.env.ATTRICAT_E2E_EXAMPLE_EXTENSION_ARCHIVE;
  if (!archive) throw new Error('E2E example extension archive is unavailable');

  await page.goto('/manage/extensions/sideload');
  await expect
    .poll(() =>
      page.evaluate(async () =>
        fetch('/api/auth/session').then((response) => response.json()),
      ),
    )
    .toMatchObject({ capabilities: { extensions_manage: true } });
  const fileInput = page.locator('input[type="file"]');
  await fileInput.setInputFiles(archive);
  await expect
    .poll(() =>
      fileInput.evaluate((input: HTMLInputElement) => input.files?.length),
    )
    .toBe(1);
  const install = page.getByRole('button', { name: 'Install archive' });
  await expect(install).toBeEnabled();
  await install.click();
  await expect(
    page.getByText(
      'Extension installed. Configure permissions before enabling it.',
    ),
  ).toBeVisible();

  await page.goto('/manage/extensions/attricat-extension-example');
  await page.getByRole('tab', { name: 'Permissions' }).click();
  const grants = page.getByRole('button', { name: 'Grant' });
  await expect(grants.first()).toBeVisible();
  while (await grants.count()) {
    const count = await grants.count();
    await grants.first().click();
    await expect.poll(() => grants.count()).toBe(count - 1);
  }
  await page.getByRole('button', { name: 'Enable' }).click();
  await expect(page.getByText('Enabled', { exact: true })).toBeVisible();
  await expect
    .poll(
      () =>
        page.evaluate(async () => {
          const runtime: Array<{ extension_id: string }> = await fetch(
            '/api/extensions/runtime',
          ).then((response) => response.json());
          return runtime.some(
            (contribution) =>
              contribution.extension_id === 'attricat-extension-example',
          );
        }),
      { timeout: 20_000 },
    )
    .toBe(true);

  // Configuration opens in a JSON editor that colors keys apart from values
  // and formats a compact document once it loses focus.
  await page.getByRole('tab', { name: 'Configuration' }).click();
  const configuration = page.getByRole('group', {
    name: 'JSON configuration',
  });
  const lines = configuration.locator('.view-line');
  await expect(configuration.locator('.monaco-editor')).toBeVisible();
  const input = configuration.getByRole('textbox', {
    name: 'JSON configuration',
  });
  await input.focus();
  await expect(input).toBeFocused();
  // A new installation starts from the one line `{}`; selecting it needs no
  // platform modifier, which macOS headless Chromium keeps from Monaco.
  await expect(lines).toHaveText(['{}']);
  await page.keyboard.press('Home');
  await page.keyboard.press('Shift+End');
  // Typing over a selection would wrap it in the auto-closed brackets.
  await page.keyboard.press('Delete');
  await page.keyboard.insertText('{"webhook_secret":"example-webhook-secret"}');
  await expect(lines).toHaveCount(1);
  await page.getByRole('heading', { name: 'Configuration' }).click();
  await expect(lines).toHaveCount(3);
  await expect(lines.nth(1)).toHaveText(
    '  "webhook_secret": "example-webhook-secret"',
  );
  const tokenColor = (text: string) =>
    lines
      .nth(1)
      .locator('span span', { hasText: text })
      .evaluate((token) => getComputedStyle(token).color);
  // Monaco colors the tokens after it renders the text, so wait for them.
  await expect
    .poll(
      async () =>
        (await tokenColor('"webhook_secret"')) !==
        (await tokenColor('"example-webhook-secret"')),
    )
    .toBe(true);
  const saved = page.waitForResponse((response) =>
    response.url().endsWith('/configure'),
  );
  await page.getByRole('button', { name: 'Save configuration' }).click();
  const response = await saved;
  expect({
    status: response.status(),
    sent: response.request().postDataJSON(),
  }).toEqual({
    status: 200,
    sent: { configuration: { webhook_secret: 'example-webhook-secret' } },
  });
  await page.reload();
  await expect(lines.nth(1)).toHaveText(
    '  "webhook_secret": "example-webhook-secret"',
  );

  const code = `formula_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Formula product',
    `attributes:
  - code: title
    value_type: string
  - code: price_net
    value_type: number
  - code: price_gross
    value_type: number
views:
  detail:
    type: stack
    extensions: [attricat-extension-example/recalculate-record]
  table:
    type: table
    extensions: [attricat-extension-example/recalculate-row]
    columns:
      - field: price_net
        renderer:
          id: attricat-extension-example.computed-number
          version: 1
          props: {precision: 2, unit: EUR}
extensions:
  attricat-extension-example:
    formulas: {price_gross: price_net * (1 + 0.23)}`,
  );
  const record = await createRecord(blueprint, [scalar('price_net', 100)]);

  await page.goto(`/records/${record.id}`);
  // Field notes come from the extension's server component, which compiles
  // its WASM on first use; the host asks again until it answers. Only the
  // formula target carries the Computed badge.
  const computed = page.getByRole('button', { name: 'Computed' });
  await expect(computed).toBeVisible({ timeout: 90_000 });
  await expect(computed).toHaveCount(1);
  await expect(page.getByText('Feeds price_gross')).toBeVisible();
  // Record actions are host-rendered buttons; choosing this one calls the
  // extension's server command, whose notification the host shows.
  await page
    .getByRole('button', { name: 'Recalculate formulas', exact: true })
    .click();
  await expect(
    page.getByText(
      /Updated \d+ computed value\(s\)\.|Computed values are up to date\./,
    ),
  ).toBeVisible({ timeout: 90_000 });

  const context = await defaultContext();
  await expect
    .poll(
      async () => {
        const preview = await request<{
          values: Record<string, { value: unknown }>;
        }>(`/records/${record.id}/resolved-preview?context_id=${context.id}`);
        return Object.values(preview.values).map(({ value }) => value);
      },
      { timeout: 120_000 },
    )
    .toContain(123);

  // The host draws the extension's cell with its precision and unit; the
  // built-in cell would show a bare number. A cold Worker start can miss the
  // first render, so reload to retry.
  await expect(async () => {
    await page.goto(`/?blueprint=${code}`);
    await expect(page.getByRole('cell', { name: /100\.00\s*EUR/ })).toBeVisible(
      { timeout: 10_000 },
    );
  }).toPass({ timeout: 60_000 });

  // A row action is an item of the row's menu; it opens the extension's
  // dialog with that one record.
  await page.getByRole('button', { name: /^Record actions for / }).click();
  await page.getByRole('menuitem', { name: 'Recalculate formulas…' }).click();
  const dialog = page.frameLocator('iframe[title="Recalculate formulas"]');
  await expect(dialog.getByText(/1 selected record/)).toBeVisible({
    timeout: 20_000,
  });
  await page
    .getByRole('dialog')
    .getByRole('button', { name: 'Close', exact: true })
    .click();
  await expect(
    page.locator('iframe[title="Recalculate formulas"]'),
  ).toHaveCount(0);
});
