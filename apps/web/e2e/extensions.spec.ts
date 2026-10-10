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
    .poll(() => fileInput.evaluate((input) => input.files?.length))
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

  const code = `formula_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Formula product',
    `[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "price_net"
value_type = "number"

[[attributes]]
code = "price_gross"
value_type = "number"

[views.table]
type = "table"
columns = [{ field = "price_net", renderer = { id = "attricat-extension-example.computed-number", version = 1, props = { precision = 2, unit = "EUR" } } }]

[extensions.attricat-extension-example.formulas]
price_gross = "price_net * (1 + 0.23)"`,
  );
  const record = await createRecord(blueprint, [scalar('price_net', 100)]);

  await page.goto(`/records/${record.id}`);
  // The decoration asks the server component for the formula index, which
  // compiles its WASM on first use.
  await expect(
    page.getByLabel('View extension content for Price gross'),
  ).toBeVisible({ timeout: 90_000 });
  await expect(
    page.getByLabel('View extension content for Price net'),
  ).toHaveCount(0);
  const action = page
    .frameLocator('iframe[title="recalculate-formulas-action"]')
    .getByRole('button', { name: 'Recalculate formulas' });
  // The sandboxed frame fetches and mounts its artifact after the page loads.
  await expect(action).toBeVisible({ timeout: 20_000 });
  await action.click();

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

  const cellFrame = page.frameLocator(
    'iframe[title="attricat-extension-example.computed-number"]',
  );
  // The Explorer shows its built-in cell when a renderer frame takes longer
  // than 1.5 s to start, which a cold artifact load can miss; reload to retry.
  await expect(async () => {
    await page.goto(`/?blueprint=${code}`);
    await expect(cellFrame.getByText(/100\.00\s*EUR/)).toBeVisible({
      timeout: 10_000,
    });
  }).toPass({ timeout: 60_000 });

  await page.getByLabel(`Record actions for ${record.id}`).click();
  await expect(page.getByLabel('Extension actions')).toBeVisible();
  await page.getByLabel('Extension actions').click();
  const rowAction = page
    .frameLocator('iframe[title="recalculate-selection-row"]')
    .getByRole('button', { name: /^Recalculate formulas/ });
  await expect(rowAction).toBeVisible();
  await rowAction.click();
  const dialog = page.frameLocator('iframe[title="Recalculate formulas"]');
  await expect(dialog.getByText('1 selected record')).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(
    page.locator('iframe[title="Recalculate formulas"]'),
  ).toHaveCount(0);
});
