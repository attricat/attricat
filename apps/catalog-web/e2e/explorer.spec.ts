import { expect, test, type Page } from '@playwright/test';
import {
  createEntity,
  createEntityBlueprint,
  createRevision,
  publishRevision,
  relationship,
  scalar,
  suffix,
} from './helpers';

const addRelationshipFilter = async (
  page: Page,
  field: string,
  option: string,
) => {
  await page.getByRole('button', { name: 'Add filter' }).click();
  const fieldDialog = page.getByRole('dialog', { name: /^Add filter/ });
  await fieldDialog.getByRole('combobox', { name: 'Field' }).click();
  await page.locator(`[role="option"][data-value="${field}"]`).click();
  const relationshipDialog = page.getByRole('dialog', { name: /^Select / });
  await relationshipDialog
    .getByRole('button', { name: `Select ${option}` })
    .click();
  await relationshipDialog.getByRole('button', { name: 'Done' }).click();
};

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
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(
    page.getByText('No entities matched this search.'),
  ).toBeVisible();

  await page.getByLabel('Query').fill('table');
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(page.getByRole('columnheader', { name: 'title' })).toBeVisible();
  await expect(page.getByRole('columnheader', { name: 'stock' })).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Table product' }).first(),
  ).toBeVisible();
  await expect(page.getByText('12', { exact: true })).toBeVisible();

  const repeatedButtonSearch = page.waitForResponse(
    (response) =>
      response.url().endsWith('/api/v1/entities/search') &&
      response.request().method() === 'POST',
  );
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await repeatedButtonSearch;

  const repeatedKeyboardSearch = page.waitForResponse(
    (response) =>
      response.url().endsWith('/api/v1/entities/search') &&
      response.request().method() === 'POST',
  );
  await page.getByLabel('Query').press('Enter');
  await repeatedKeyboardSearch;
});

test('saves an Explorer search and restores it through a short URL', async ({
  page,
}) => {
  const code = `saved_${suffix()}`;
  await createEntityBlueprint(
    code,
    'Saved products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
  );
  await page.goto(`/?blueprint=${code}&query=example`);
  await page.getByRole('button', { name: 'Saved searches' }).click();
  const savedSearches = page.getByRole('dialog', { name: 'Saved searches' });
  await expect(
    savedSearches.getByRole('button', { name: 'Save current search' }),
  ).toBeEnabled();
  await savedSearches
    .getByRole('button', { name: 'Save current search' })
    .click();
  await page
    .getByRole('dialog', { name: 'Save search' })
    .getByRole('textbox', { name: 'Name' })
    .fill('My saved products');
  await page
    .getByRole('dialog', { name: 'Save search' })
    .getByRole('button', { name: 'Save search' })
    .click();
  await expect
    .poll(() => new URL(page.url()).searchParams.get('savedView'))
    .toMatch(/^[0-9a-f-]{36}$/);
  const url = page.url();
  await page.reload();
  await expect(page.getByLabel('Query')).toHaveValue('example');
  await expect(
    page.getByRole('button', { name: 'Share search' }),
  ).toBeEnabled();
  const activator = page.getByRole('button', {
    name: 'Saved search: My saved products',
  });
  await expect(activator).toHaveAttribute('aria-pressed', 'true');
  await activator.click();
  const savedSearchDialog = page.getByRole('dialog', {
    name: 'Saved searches',
  });
  await expect(
    savedSearchDialog.getByRole('button', {
      name: 'Delete saved search My saved products',
    }),
  ).toBeVisible();
  const filter = savedSearchDialog.getByRole('searchbox', {
    name: 'Filter saved searches',
  });
  await filter.fill(`no match ${code}`);
  await expect(
    savedSearchDialog.getByText('No saved searches match this filter.'),
  ).toBeVisible();
  await filter.fill('MY SAVED PROD');
  await expect(
    savedSearchDialog.getByText('My saved products').first(),
  ).toBeVisible();
  await savedSearchDialog
    .getByRole('button', { name: 'Clear saved search' })
    .click();
  await expect(
    page.getByRole('button', { name: 'Saved searches' }),
  ).toHaveAttribute('aria-pressed', 'false');
  await expect(page.getByLabel('Query')).toHaveValue('');
  expect(new URL(page.url()).searchParams.get('savedView')).toBeNull();
  await page.goto(url);
  await expect(page.getByLabel('Query')).toHaveValue('example');
  expect(page.url()).toBe(url);
});

test('applies and removes an attribute filter on mobile', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const code = `attribute_filter_${suffix()}`;
  const blueprint = await createEntityBlueprint(
    code,
    'Attribute filter products',
    `[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "stock"
value_type = "integer"`,
    { views: '[views.table]\ntype = "table"\nfields = ["title", "stock"]' },
  );
  await createEntity(blueprint, [
    scalar('title', 'In stock'),
    scalar('stock', 12),
  ]);
  await createEntity(blueprint, [
    scalar('title', 'Low stock'),
    scalar('stock', 2),
  ]);

  await page.goto(`/?blueprint=${code}`);
  await expect(page.getByText('2 results')).toBeVisible();
  await page.getByRole('button', { name: 'Open navigation' }).click();
  await expect(
    page.getByRole('combobox', { name: /select a blueprint/i }),
  ).toHaveCount(1);
  const mobileFilters = page.locator('.MuiDrawer-paper aside');
  await expect(
    mobileFilters.getByRole('heading', { name: 'Filters' }),
  ).toBeVisible();
  await mobileFilters.getByRole('button', { name: 'Add filter' }).click();
  const filterDialog = page.getByRole('dialog', {
    name: /^Add filter/,
  });
  await filterDialog
    .getByRole('combobox', { name: 'Field', exact: true })
    .click();
  await page.getByRole('option', { name: 'stock' }).click();
  await filterDialog.getByLabel('Operator').click();
  await page.getByRole('option', { name: 'Greater than', exact: true }).click();
  await filterDialog.getByLabel('Value').fill('5');
  await filterDialog.getByLabel('Value').press('Enter');

  await expect(filterDialog).toBeHidden();
  await expect(mobileFilters.getByText('stock > "5"')).toBeVisible();
  await expect(page.getByText('1 result')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(mobileFilters).toBeHidden();
  await expect(
    page.getByRole('cell', { name: 'In stock' }).first(),
  ).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Low stock' })).toBeHidden();
  await expect
    .poll(() => new URL(page.url()).searchParams.get('attributeFilters'))
    .toContain('stock');

  await page.getByRole('button', { name: 'Open navigation' }).click();
  await expect(mobileFilters).toBeVisible();
  await mobileFilters.getByText('stock > "5"').click();
  const editDialog = page.getByRole('dialog', {
    name: /^Edit filter/,
  });
  await expect(editDialog.getByLabel('Value')).toHaveValue('5');
  await editDialog.getByLabel('Value').fill('10');
  await editDialog.getByLabel('Value').press('Enter');
  await expect(editDialog).toBeHidden();
  await expect(mobileFilters.getByText('stock > "10"')).toBeVisible();

  await mobileFilters
    .locator('.MuiChip-root')
    .filter({ hasText: 'stock > "10"' })
    .locator('.MuiChip-deleteIcon')
    .click();
  await expect(page.getByText('2 results')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(mobileFilters).toBeHidden();
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
  await addRelationshipFilter(page, 'category', 'Shoes');

  await expect
    .poll(() => new URL(page.url()).searchParams.get('relationshipFacets'))
    .toContain(child.id);
  await expect(page.getByText('category: 1 selected')).toBeVisible();
  await expect(page.getByText('1 result')).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Running shoe' })).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Canvas bag' })).toBeHidden();

  await page
    .getByLabel('Remove relationship filter category: 1 selected')
    .click();
  await expect(page.getByText('2 results')).toBeVisible();
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
target_blueprint = "${colorCode}"
cardinality = "one"`,
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
  await addRelationshipFilter(page, 'color', 'Red');

  await expect
    .poll(() => new URL(page.url()).searchParams.get('relationshipFacets'))
    .toContain(red.id);
  await expect(page.getByText('1 result')).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Red shirt' })).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Blue shirt' })).toBeHidden();
});

test('intersects selections from multiple relationship facets', async ({
  page,
}) => {
  const colorCode = `multi_color_${suffix()}`;
  const sizeCode = `multi_size_${suffix()}`;
  const productCode = `multi_product_${suffix()}`;
  const color = await createEntityBlueprint(
    colorCode,
    'Colors',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  const size = await createEntityBlueprint(
    sizeCode,
    'Sizes',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  const red = await createEntity(color, [scalar('title', 'Red')]);
  const blue = await createEntity(color, [scalar('title', 'Blue')]);
  const small = await createEntity(size, [scalar('title', 'Small')]);
  const large = await createEntity(size, [scalar('title', 'Large')]);
  const product = await createEntityBlueprint(
    productCode,
    'Products with multiple facets',
    `[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "color"
value_type = "relationship"
target_blueprint = "${colorCode}"

[[attributes]]
code = "size"
value_type = "relationship"
target_blueprint = "${sizeCode}"`,
  );
  await createEntity(product, [
    scalar('title', 'Red large shirt'),
    relationship('color', red.id),
    relationship('size', large.id),
  ]);
  await createEntity(product, [
    scalar('title', 'Red small shirt'),
    relationship('color', red.id),
    relationship('size', small.id),
  ]);
  await createEntity(product, [
    scalar('title', 'Blue large shirt'),
    relationship('color', blue.id),
    relationship('size', large.id),
  ]);

  await page.goto(`/?blueprint=${productCode}`);
  await addRelationshipFilter(page, 'color', 'Red');
  await addRelationshipFilter(page, 'size', 'Large');

  await expect(page.getByText('1 result')).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Red large shirt' }),
  ).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Red small shirt' }),
  ).toBeHidden();
  await expect(
    page.getByRole('cell', { name: 'Blue large shirt' }),
  ).toBeHidden();
  await expect
    .poll(() => new URL(page.url()).searchParams.get('relationshipFacets'))
    .toContain(red.id);
  await expect
    .poll(() => new URL(page.url()).searchParams.get('relationshipFacets'))
    .toContain(large.id);
});

test('loads additional explorer search pages', async ({ page }) => {
  test.setTimeout(60_000);
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

  await expect(page.getByText('52 results', { exact: true })).toBeVisible();
  const resultsContainer = page.locator('.MuiTableContainer-root');
  await resultsContainer.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  const loadMore = page.getByRole('button', { name: 'Load more' });
  await expect(loadMore).toBeVisible();
  await loadMore.click();
  await expect(page.getByRole('button', { name: 'Loading...' })).toBeDisabled();
  await expect.poll(() => searchRequests).toBe(2);
  await expect(page.getByText('52 results', { exact: true })).toBeVisible();
  expect(await resultsContainer.getByRole('row').count()).toBeLessThan(51);
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
  await expect(
    page.getByText(
      'Showing current-version entities. 1 older-version entity is hidden.',
    ),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Show all versions' }),
  ).toBeVisible();
  await expect(
    page.getByRole('link', { name: 'Review migrations' }),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: `View entity ID ${oldEntity.id}` }),
  ).toBeHidden();

  await page.getByRole('button', { name: 'Show all versions' }).click();
  await expect(page).toHaveURL(/allVersions=true/);
  await expect(
    page.getByRole('button', { name: `View entity ID ${oldEntity.id}` }),
  ).toBeVisible();

  await page.getByLabel('Version scope').click();
  await page.getByRole('option', { name: 'Version 1' }).click();
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`blueprint=${code}.*version=1`));
  await expect(page.getByRole('cell', { name: 'v1 · Outdated' })).toBeVisible();
});
