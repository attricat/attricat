import { expect, test } from '@playwright/test';
import {
  createEntity,
  createEntityBlueprint,
  createRevision,
  publishRevision,
  scalar,
  suffix,
} from './helpers';

test('shows explorer empty states and configured table fields', async ({
  page,
}) => {
  const code = `explorer_${suffix()}`;
  const blueprint = await createEntityBlueprint(
    code,
    'Explorer products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]\n\n[[attributes]]\ncode = "stock"\nvalue_type = "integer"',
    { views: '[views.table]\ntype = "table"\nfields = ["title", "stock"]' },
  );
  await createEntity(blueprint, [
    scalar('title', 'Table product'),
    scalar('stock', 12),
  ]);

  await page.goto('/');
  await expect(
    page.getByText('Enter a blueprint code to start exploring.'),
  ).toBeVisible();
  await page.getByLabel('Select a Blueprint').click();
  await page
    .getByRole('option', { name: `Explorer products (${code})` })
    .click();
  await page.getByLabel('Query').fill('missing');
  await page.getByRole('button', { name: 'Search' }).click();
  await expect(
    page.getByText('No entities matched this search.'),
  ).toBeVisible();

  await page.getByLabel('Query').fill('table');
  await page.getByRole('button', { name: 'Search' }).click();
  await expect(page.getByRole('columnheader', { name: 'title' })).toBeVisible();
  await expect(page.getByRole('columnheader', { name: 'stock' })).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Table product' }).first(),
  ).toBeVisible();
  await expect(page.getByText('12', { exact: true })).toBeVisible();
});

test('searches a requested blueprint version and identifies outdated entities', async ({
  page,
}) => {
  const code = `versions_${suffix()}`;
  const firstDefinition = `format_version = 1
code = "${code}"
name = "Versioned products"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]`;
  const first = await createEntityBlueprint(
    code,
    'Versioned products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
  );
  const oldEntity = await createEntity(first, [scalar('title', 'Old product')]);
  const second = await createRevision(
    first.blueprint.id,
    `${firstDefinition}

[[attributes]]
code = "sku"
value_type = "string"`,
  );
  await publishRevision(second);

  await page.goto(`/?blueprint=${code}`);
  await expect(page.getByText('v2')).toBeVisible();
  await expect(page.getByText('Outdated')).toBeVisible();
  await expect(page.getByRole('link', { name: oldEntity.id })).toBeVisible();

  await page.getByLabel('Version').fill('1');
  await page.getByRole('button', { name: 'Search' }).click();
  await expect(page).toHaveURL(new RegExp(`blueprint=${code}.*version=1`));
  await expect(page.getByRole('cell', { name: 'v1 · Outdated' })).toBeVisible();
});
