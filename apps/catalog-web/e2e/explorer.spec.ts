import { expect, test } from '@playwright/test';
import {
  createEntity,
  createEntityBlueprint,
  createRevision,
  publishRevision,
  relationship,
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

test('filters explorer results with a relationship hierarchy facet', async ({
  page,
}) => {
  const categoryCode = `facet_category_${suffix()}`;
  const productCode = `facet_product_${suffix()}`;
  const category = await createEntityBlueprint(
    categoryCode,
    'Facet categories',
    `[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "${categoryCode}"`,
  );
  const parent = await createEntity(category, [scalar('title', 'Departments')]);
  const child = await createEntity(category, [
    scalar('title', 'Shoes'),
    relationship('parent', parent.id),
  ]);
  const other = await createEntity(category, [scalar('title', 'Accessories')]);
  const product = await createEntityBlueprint(
    productCode,
    'Faceted products',
    `[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "${categoryCode}"`,
  );
  await createEntity(product, [
    scalar('title', 'Running shoe'),
    relationship('category', child.id),
  ]);
  await createEntity(product, [
    scalar('title', 'Canvas bag'),
    relationship('category', other.id),
  ]);

  await page.goto(`/?blueprint=${productCode}`);
  await expect(page.getByText('2 results')).toBeVisible();
  await page.getByLabel('Relationship').click();
  await page.getByRole('option', { name: 'category' }).click();
  await expect(
    page.getByRole('checkbox', { name: 'Departments (1)' }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Expand Departments' }).click();
  await page.getByRole('checkbox', { name: 'Shoes (1)' }).check();

  await expect
    .poll(() => new URL(page.url()).searchParams.get('categories'))
    .toBe(JSON.stringify([child.id]));
  await expect(page.getByText('1 result')).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Running shoe' })).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Canvas bag' })).toBeHidden();
});

test('filters explorer results with a one-level relationship facet', async ({
  page,
}) => {
  const colorCode = `facet_color_${suffix()}`;
  const productCode = `facet_color_product_${suffix()}`;
  const color = await createEntityBlueprint(
    colorCode,
    'Facet colors',
    `[[attributes]]
code = "title"
value_type = "string"`,
  );
  const red = await createEntity(color, [scalar('title', 'Red')]);
  const blue = await createEntity(color, [scalar('title', 'Blue')]);
  const product = await createEntityBlueprint(
    productCode,
    'Color products',
    `[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "color"
value_type = "relationship"
target_blueprint = "${colorCode}"`,
  );
  await createEntity(product, [
    scalar('title', 'Red shirt'),
    relationship('color', red.id),
  ]);
  await createEntity(product, [
    scalar('title', 'Blue shirt'),
    relationship('color', blue.id),
  ]);

  await page.goto(`/?blueprint=${productCode}`);
  await page.getByLabel('Relationship').click();
  await page.getByRole('option', { name: 'color' }).click();
  await expect(page.getByRole('checkbox', { name: 'Red (1)' })).toBeVisible();
  await expect(page.getByRole('checkbox', { name: 'Blue (1)' })).toBeVisible();
  await expect(page.getByText('Facet options')).toBeVisible();
  await page.getByRole('checkbox', { name: 'Red (1)' }).check();

  await expect
    .poll(() => new URL(page.url()).searchParams.get('categories'))
    .toBe(JSON.stringify([red.id]));
  await expect(page.getByText('1 result')).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Red shirt' })).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Blue shirt' })).toBeHidden();
});

test('loads additional explorer search pages', async ({ page }) => {
  const code = `pagination_${suffix()}`;
  const blueprint = await createEntityBlueprint(
    code,
    'Paginated products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
  );
  for (let index = 0; index < 52; index += 1) {
    await createEntity(blueprint, [
      scalar('title', `Pagination product ${index}`),
    ]);
  }

  let searchRequests = 0;
  await page.route('**/api/v1/entities/search', async (route) => {
    searchRequests += 1;
    await new Promise((resolve) => setTimeout(resolve, 300));
    await route.continue();
  });
  await page.goto(`/?blueprint=${code}&query=Pagination`);

  await expect(page.getByText(/25 results$/)).toBeVisible();
  const resultsContainer = page.getByLabel('Explorer results');
  await resultsContainer.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  const loadMore = page.getByRole('button', { name: 'Load more' });
  await expect(loadMore).toBeVisible();
  await loadMore.click();
  await expect(page.getByRole('button', { name: 'Loading...' })).toBeDisabled();
  await expect.poll(() => searchRequests).toBe(2);
  await expect(page.getByText(/50 results$/)).toBeVisible();
  expect(await resultsContainer.getByRole('row').count()).toBeLessThan(51);
  await resultsContainer.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(loadMore).toBeVisible();
  await loadMore.click();
  await expect(page.getByText(/52 results$/)).toBeVisible();
  await expect(loadMore).toBeHidden();
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
