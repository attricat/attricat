import { expect, test } from '@playwright/test';
import {
  createBlueprint,
  createEntityBlueprint,
  createRevision,
  suffix,
} from './helpers';

test('browses, filters, and compares blueprint revisions', async ({ page }) => {
  const code = `catalogue_${suffix()}`;
  const first = await createEntityBlueprint(
    code,
    'Catalogue product',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  await createRevision(
    first.blueprint.id,
    `format_version = 1
code = "${code}"
name = "Catalogue product revision"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["sku"]

[[attributes]]
code = "sku"
value_type = "string"`,
  );
  await createBlueprint(
    `format_version = 1\ncode = "mixin_${suffix()}"\nname = "Catalogue mixin"\nkind = "mixin"\n\n[[attributes]]\ncode = "label"\nvalue_type = "string"`,
  );

  await page.goto('/blueprints');
  await page.getByLabel('Filter blueprints').fill('draft');
  await expect(page.getByText('Catalogue product revision')).toBeVisible();
  await expect(page.getByText(code)).toBeVisible();
  await page.getByLabel('Filter blueprints').fill('mixin');
  await expect(page.getByText('Catalogue mixin')).toBeVisible();
  await page.getByLabel('Filter blueprints').fill(code);
  await page.getByRole('link', { name: 'Catalogue product revision' }).click();

  await expect(
    page.getByRole('heading', { name: 'Catalogue product revision' }),
  ).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Revision history' }),
  ).toBeVisible();
  await expect(page.getByRole('cell', { name: 'v2' })).toBeVisible();
  await page.getByRole('button', { name: 'Compare definitions' }).click();
  await page.getByLabel('Left version').click();
  await page.getByRole('option', { name: 'v1 (published)' }).click();
  await expect(page.getByText('code = "title"').first()).toBeVisible();
  await page.getByRole('tab', { name: /Attributes/ }).click();
  await expect(page.getByText('title', { exact: true })).toBeVisible();
});
