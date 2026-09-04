import { expect, test } from '@playwright/test';
import {
  createBlueprint,
  createEntityBlueprint,
  createRevision,
  suffix,
} from './helpers';

test('previews blueprint views and inspects revision metadata', async ({
  page,
}) => {
  const code = `preview_${suffix()}`;
  const blueprint = await createEntityBlueprint(
    code,
    'Preview product',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
    {
      entitySchema:
        '{"type":"object","required":["title"],"properties":{"title":{"type":"string"}}}',
      views:
        '[views.edit]\ntype = "stack"\nchildren = [{ type = "field", field = "title" }]\n\n[views.detail]\ntype = "stack"\nchildren = [{ type = "heading", text = "Preview detail" }, { type = "field", field = "title" }]\n\n[views.table]\ntype = "table"\nfields = ["title"]',
    },
  );

  await page.goto(`/manage/blueprints/${blueprint.blueprint.id}`);
  await page.getByRole('tab', { name: 'Views' }).click();
  await expect(
    page.getByText('Sandbox values stay in this page and are never saved.'),
  ).toBeVisible();
  await page.getByRole('tab', { name: 'edit', exact: true }).click();
  await page.getByLabel('title').fill('Sandbox title');
  await page.getByRole('tab', { name: 'detail', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Preview detail' }),
  ).toBeVisible();
  await expect(page.getByText('Sandbox title')).toBeVisible();
  await page.getByRole('tab', { name: 'table', exact: true }).click();
  await expect(page.getByRole('cell', { name: 'Sandbox title' })).toBeVisible();

  await page.getByRole('tab', { name: 'View definition' }).click();
  await expect(page.getByText(/"detail"/)).toBeVisible();
  await page.getByRole('tab', { name: 'Entity schema' }).click();
  await expect(page.getByText(/"required"/)).toBeVisible();
  await page.getByRole('tab', { name: 'Includes' }).click();
  await expect(page.getByText('[]')).toBeVisible();
});

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

  await page.goto('/manage/blueprints');
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
