import { expect, test } from '@playwright/test';
import {
  createEntity,
  createEntityBlueprint,
  defaultContext,
  request,
  scalar,
  suffix,
} from './helpers';

test('side-loads the example extension and recalculates a blueprint formula', async ({
  page,
}) => {
  const archive = process.env.CATALOG_E2E_EXAMPLE_EXTENSION_ARCHIVE;
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
  await expect(page.getByText('enabled', { exact: true })).toBeVisible();

  const code = `formula_${suffix()}`;
  const blueprint = await createEntityBlueprint(
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
columns = [{ field = "price_net", renderer = { id = "attricat-extension-example.table-cell", version = 1 } }]

[extensions.attricat-extension-example.formulas]
price_gross = "price_net * (1 + 0.23)"`,
  );
  const entity = await createEntity(blueprint, [scalar('price_net', 100)]);

  await page.goto(`/entities/${entity.id}`);
  await expect(
    page.getByLabel('View extension content for price gross'),
  ).toBeVisible();
  await expect(
    page.getByLabel('View extension content for price net'),
  ).toHaveCount(0);
  const action = page
    .frameLocator('iframe[title="recalculate-formulas-action"]')
    .getByRole('button', { name: 'Recalculate formulas' });
  await expect(action).toBeVisible();
  await action.click();

  const context = await defaultContext();
  await expect
    .poll(
      async () => {
        const preview = await request<{
          values: Record<string, { value: unknown }>;
        }>(`/entities/${entity.id}/resolved-preview?context_id=${context.id}`);
        return Object.values(preview.values).some(({ value }) => value === 123);
      },
      { timeout: 10_000 },
    )
    .toBe(true);

  await page.goto('/');
  await page.getByLabel('Select a Blueprint').click();
  await page.getByRole('option', { name: `Formula product (${code})` }).click();
  await page.getByRole('button', { name: 'Search' }).click();
  const cellFrame = page.frameLocator(
    'iframe[title="attricat-extension-example.table-cell"]',
  );
  await expect(cellFrame.getByText('Example cell: 100')).toBeVisible();

  await page.getByLabel(`Entity actions for ${entity.id}`).click();
  await expect(page.getByLabel('Extension actions')).toBeVisible();
  await page.getByLabel('Extension actions').click();
  const rowAction = page
    .frameLocator('iframe[title="example-row-action"]')
    .getByRole('button', { name: 'Example row action' });
  await expect(rowAction).toBeVisible();
  await rowAction.click();
  await expect(page.getByText(`Row action for ${entity.id}`)).toBeVisible();
});
