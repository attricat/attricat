import { expect, test, type Page } from '@playwright/test';
import {
  createRecord,
  createRecordBlueprint,
  createRevision,
  defaultContext,
  publishRevision,
  relationship,
  request,
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
  await relationshipDialog.getByRole('checkbox', { name: option }).check();
  await relationshipDialog.getByRole('button', { name: 'Done' }).click();
};

test('shows explorer empty states and configured table fields', async ({
  page,
}) => {
  const code = `explorer_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Explorer products',
    'attributes:\n  - code: title\n    value_type: string\n    tags: [searchable]\n  - code: stock\n    value_type: integer',
    {
      views:
        'views:\n  table:\n    type: table\n    columns:\n      - {field: title}\n      - {field: stock}',
    },
  );
  await createRecord(blueprint, [
    scalar('title', 'Table product'),
    scalar('stock', 12),
  ]);

  await page.goto('/');
  await expect(
    page
      .getByRole('region', { name: 'Explorer products' })
      .getByRole('cell', { name: 'Table product', exact: true })
      .first(),
  ).toBeVisible();
  await page.getByLabel('Select a Blueprint').click();
  await page
    .getByRole('option', { name: `Explorer products (${code})` })
    .click();
  await page.getByLabel('Query').fill('missing');
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(page.getByText('No records matched this search.')).toBeVisible();

  await page.getByLabel('Query').fill('table');
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(page.getByRole('columnheader', { name: 'title' })).toBeVisible();
  await expect(page.getByRole('columnheader', { name: 'stock' })).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Table product', exact: true }).first(),
  ).toBeVisible();
  await expect(page.getByText('12', { exact: true })).toBeVisible();

  const repeatedButtonSearch = page.waitForResponse(
    (response) =>
      response.url().endsWith('/api/v1/records/search') &&
      response.request().method() === 'POST',
  );
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await repeatedButtonSearch;

  const repeatedKeyboardSearch = page.waitForResponse(
    (response) =>
      response.url().endsWith('/api/v1/records/search') &&
      response.request().method() === 'POST',
  );
  await page.getByLabel('Query').press('Enter');
  await repeatedKeyboardSearch;
});

test('reorders Explorer columns by dragging their handles', async ({
  page,
}) => {
  const code = `explorer_columns_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Explorer column order',
    'attributes:\n  - code: title\n    value_type: string\n  - code: stock\n    value_type: integer',
    {
      views:
        'views:\n  table:\n    type: table\n    columns:\n      - {field: title}\n      - {field: stock}',
    },
  );
  await createRecord(blueprint, [
    scalar('title', 'Column product'),
    scalar('stock', 3),
  ]);
  await page.goto(`/?blueprint=${code}`);
  const headers = async () =>
    (await page.getByRole('columnheader').allInnerTexts()).map((text) =>
      text.trim(),
    );
  await expect(page.getByRole('columnheader', { name: 'stock' })).toBeVisible();
  const before = await headers();
  expect(before.indexOf('title')).toBeLessThan(before.indexOf('stock'));

  await page.getByRole('button', { name: 'Columns' }).click();
  const list = page
    .getByRole('dialog', { name: 'Columns' })
    .getByRole('list', { name: 'Columns' });
  const handle = await list
    .getByRole('button', { name: 'Drag to reorder title' })
    .boundingBox();
  const target = await list
    .getByRole('listitem')
    .filter({ hasText: 'stock' })
    .boundingBox();
  if (!handle || !target) throw new Error('column rows are not visible');
  const x = handle.x + handle.width / 2;
  await page.mouse.move(x, handle.y + handle.height / 2);
  await page.mouse.down();
  // Move in steps so the pointer sensor activates and sorting follows.
  await page.mouse.move(x, handle.y + handle.height, { steps: 5 });
  await page.mouse.move(x, target.y + target.height * 0.75, { steps: 20 });
  await page.mouse.up();

  await expect
    .poll(async () => {
      const rows = (await list.getByRole('listitem').allInnerTexts()).map(
        (text) => text.trim(),
      );
      return rows.indexOf('stock') < rows.indexOf('title');
    })
    .toBe(true);
  await page.getByRole('button', { name: 'Done' }).click();
  await expect
    .poll(async () => {
      const after = await headers();
      return after.indexOf('stock') < after.indexOf('title');
    })
    .toBe(true);
});

test('saves an Explorer search and restores it through a short URL', async ({
  page,
}) => {
  const code = `saved_${suffix()}`;
  const savedName = `My saved products ${code}`;
  await createRecordBlueprint(
    code,
    'Saved products',
    'attributes:\n  - code: title\n    value_type: string\n    tags: [searchable]',
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
    .fill(savedName);
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
    name: `Saved search: ${savedName}`,
  });
  await expect(activator).toHaveAttribute('aria-pressed', 'true');
  await activator.click();
  const savedSearchDialog = page.getByRole('dialog', {
    name: 'Saved searches',
  });
  await expect(
    savedSearchDialog.getByRole('button', {
      name: `Delete saved search ${savedName}`,
    }),
  ).toBeVisible();
  const filter = savedSearchDialog.getByRole('searchbox', {
    name: 'Filter saved searches',
  });
  await filter.fill(`no match ${code}`);
  await expect(
    savedSearchDialog.getByText('No saved searches match this filter.'),
  ).toBeVisible();
  // The filter ignores case.
  await filter.fill(savedName.toUpperCase());
  await expect(savedSearchDialog.getByText(savedName)).toBeVisible();
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

test('prepares an equality filter from a hovered table cell', async ({
  page,
}) => {
  const code = `cell_filter_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Cell filter products',
    `attributes:
  - code: title
    value_type: string
  - code: color
    value_type: string`,
    {
      views:
        'views:\n  table:\n    type: table\n    columns:\n      - {field: title}\n      - {field: color}',
    },
  );
  await createRecord(blueprint, [
    scalar('title', 'Red laptop'),
    scalar('color', 'red'),
  ]);
  await createRecord(blueprint, [
    scalar('title', 'Blue laptop'),
    scalar('color', 'blue'),
  ]);
  await page.goto(`/?blueprint=${code}`);
  await expect(page.getByText('2 results')).toBeVisible();
  const filterButton = page.getByRole('button', {
    name: 'Filter by color = red…',
  });
  // Shortcuts mount only in the row under the pointer or keyboard focus.
  await expect(filterButton).toHaveCount(0);
  // Keyboard focus entering the row through its record link reaches them.
  await page.getByRole('link', { name: /Red laptop/ }).focus();
  for (let step = 0; step < 6; step += 1) {
    if (
      await filterButton
        .evaluate((button) => button === document.activeElement)
        .catch(() => false)
    )
      break;
    await page.keyboard.press('Tab');
  }
  await expect(filterButton).toBeFocused();
  await filterButton.blur();
  await expect(filterButton).toHaveCount(0);
  await page.getByRole('cell').filter({ hasText: /^red$/ }).hover();
  await expect(filterButton).toHaveCSS('opacity', '1');
  await filterButton.click();
  const dialog = page.getByRole('dialog', { name: 'Add filter' });
  await expect(dialog.getByRole('combobox', { name: 'Field' })).toHaveValue(
    /color/,
  );
  await expect(dialog.getByRole('textbox', { name: 'Value' })).toHaveValue(
    'red',
  );
  await expect(page.getByText('2 results')).toBeVisible();
  expect(new URL(page.url()).searchParams.get('attributeFilters')).toBeNull();
  await dialog.getByRole('button', { name: 'Add filter' }).click();
  await expect(page.getByText('1 result', { exact: true })).toBeVisible();
  await expect
    .poll(() => new URL(page.url()).searchParams.get('attributeFilters'))
    .toContain('"operator":"eq"');
});

test('applies and removes an attribute filter on mobile', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const code = `attribute_filter_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Attribute filter products',
    `attributes:
  - code: title
    value_type: string
  - code: stock
    value_type: integer`,
    {
      views:
        'views:\n  table:\n    type: table\n    columns:\n      - {field: title}\n      - {field: stock}',
    },
  );
  await createRecord(blueprint, [
    scalar('title', 'In stock'),
    scalar('stock', 12),
  ]);
  await createRecord(blueprint, [
    scalar('title', 'Low stock'),
    scalar('stock', 2),
  ]);

  await page.goto(`/?blueprint=${code}`);
  await expect(page.getByText('2 results')).toBeVisible();
  await expect(
    page.getByRole('combobox', { name: /select a blueprint/i }),
  ).toHaveCount(1);
  const filters = page.getByRole('group', { name: 'Filters' });
  await filters.getByRole('button', { name: 'Add filter' }).click();
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
  await expect(filters.getByText('stock > "5"')).toBeVisible();
  await expect(page.getByText('1 result')).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'In stock', exact: true }).first(),
  ).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Low stock', exact: true }),
  ).toBeHidden();
  await expect
    .poll(() => new URL(page.url()).searchParams.get('attributeFilters'))
    .toContain('stock');

  await filters.getByText('stock > "5"').click();
  const editDialog = page.getByRole('dialog', {
    name: /^Edit filter/,
  });
  await expect(editDialog.getByLabel('Value')).toHaveValue('5');
  await editDialog.getByLabel('Value').fill('10');
  await editDialog.getByLabel('Value').press('Enter');
  await expect(editDialog).toBeHidden();
  await expect(filters.getByText('stock > "10"')).toBeVisible();

  await filters
    .locator('.MuiChip-root')
    .filter({ hasText: 'stock > "10"' })
    .locator('.MuiChip-deleteIcon')
    .click();
  await expect(page.getByText('2 results')).toBeVisible();
  await expect(filters.locator('.MuiChip-root')).toHaveCount(0);

  // The selection toolbar wraps instead of widening the page.
  await page.getByRole('button', { name: 'Select records' }).click();
  await page
    .getByRole('checkbox', { name: /^Select / })
    .nth(1)
    .check();
  await expect(page.getByRole('button', { name: /1 selected/ })).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Columns', exact: true }),
  ).toBeInViewport({ ratio: 1 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
});

test('filters explorer results with a relationship hierarchy facet', async ({
  page,
}) => {
  const categoryCode = `facet_category_${suffix()}`;
  const productCode = `facet_product_${suffix()}`;
  const category = await createRecordBlueprint(
    categoryCode,
    'Facet categories',
    `attributes:
  - code: title
    value_type: string
  - code: parent
    value_type: relationship
    target_blueprint: '${categoryCode}'`,
  );
  const parent = await createRecord(category, [scalar('title', 'Departments')]);
  const child = await createRecord(category, [
    scalar('title', 'Shoes'),
    relationship('parent', parent.id),
  ]);
  const other = await createRecord(category, [scalar('title', 'Accessories')]);
  const product = await createRecordBlueprint(
    productCode,
    'Faceted products',
    `attributes:
  - code: title
    value_type: string
    tags: [searchable]
  - code: category
    value_type: relationship
    target_blueprint: '${categoryCode}'`,
  );
  await createRecord(product, [
    scalar('title', 'Running shoe'),
    relationship('category', child.id),
  ]);
  await createRecord(product, [
    scalar('title', 'Canvas bag'),
    relationship('category', other.id),
  ]);

  await page.goto(`/?blueprint=${productCode}`);
  await expect(page.getByText('2 results')).toBeVisible();
  await addRelationshipFilter(page, 'category', 'Shoes');

  await expect
    .poll(() => new URL(page.url()).searchParams.get('relationshipFacets'))
    .toContain(child.id);
  const categoryChip = page
    .getByRole('group', { name: 'Filters' })
    .locator('.MuiChip-root')
    .filter({ hasText: /^category: / });
  await expect(categoryChip).toBeVisible();
  await expect(page.getByText('1 result')).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Running shoe', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Canvas bag', exact: true }),
  ).toBeHidden();

  await categoryChip
    .getByLabel(/^Remove relationship filter category: /)
    .click();
  await expect(page.getByText('2 results')).toBeVisible();
});

test('browses explorer results through a category tree rail', async ({
  page,
}) => {
  const categoryCode = `rail_category_${suffix()}`;
  const productCode = `rail_product_${suffix()}`;
  const category = await createRecordBlueprint(
    categoryCode,
    'Rail categories',
    `attributes:
  - code: title
    value_type: string
  - code: parent
    value_type: relationship
    target_blueprint: '${categoryCode}'
    tree: true
    context_editable: default`,
  );
  const departments = await createRecord(category, [
    scalar('title', 'Departments'),
  ]);
  const shoes = await createRecord(category, [
    scalar('title', 'Shoes'),
    relationship('parent', departments.id),
  ]);
  const accessories = await createRecord(category, [
    scalar('title', 'Accessories'),
  ]);
  const product = await createRecordBlueprint(
    productCode,
    'Rail products',
    `attributes:
  - code: title
    value_type: string
  - code: categories
    value_type: relationship
    target_blueprint: '${categoryCode}'`,
  );
  await createRecord(product, [
    scalar('title', 'Running shoe'),
    relationship('categories', shoes.id),
  ]);
  await createRecord(product, [
    scalar('title', 'Canvas bag'),
    relationship('categories', accessories.id),
  ]);

  await page.goto(`/?blueprint=${productCode}`);
  const rail = page.getByRole('complementary', {
    name: 'Browse by categories',
  });
  await expect(
    rail.getByRole('button', { name: /^Accessories/ }),
  ).toBeVisible();
  await rail.getByRole('button', { name: 'Expand Departments' }).click();
  await expect(rail.getByRole('button', { name: /^Shoes/ })).toBeVisible();

  // Choosing a parent also matches records assigned to its descendants.
  await rail.getByRole('button', { name: /^Departments/ }).click();
  await expect(page.getByText('1 result')).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Running shoe', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Canvas bag', exact: true }),
  ).toBeHidden();

  // A selection loaded with the page opens the branches leading to it.
  await rail.getByRole('button', { name: /^Shoes/ }).click();
  await page.reload();
  await expect(
    rail.getByRole('button', { name: 'Collapse Departments' }),
  ).toBeVisible();
  await expect(rail.getByRole('button', { name: /^Shoes/ })).toHaveAttribute(
    'aria-current',
    'true',
  );
  await rail.getByRole('button', { name: 'All' }).click();
  await expect(page.getByText('2 results')).toBeVisible();

  // Hiding the rail is remembered for the blueprint.
  await rail.getByRole('button', { name: 'Hide browse panel' }).click();
  await expect(rail).toBeHidden();
  await page.goto(`/?blueprint=${productCode}`);
  await expect(page.getByText('2 results')).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Show browse panel' }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Show browse panel' }).click();
  await expect(rail).toBeVisible();

  // The category blueprint browses its own tree: a node matches itself and
  // everything below it.
  await page.goto(`/?blueprint=${categoryCode}`);
  const ownRail = page.getByRole('complementary', {
    name: 'Browse by Rail categories',
  });
  await expect(page.getByText('3 results')).toBeVisible();
  await ownRail.getByRole('button', { name: /^Departments/ }).click();
  await expect(page.getByText('2 results')).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Departments', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Shoes', exact: true }),
  ).toBeVisible();
  await expect(page.getByText('Rail categories: Departments')).toBeVisible();

  // A blueprint can keep the rail closed even for a tree field.
  const closedCode = `rail_closed_product_${suffix()}`;
  await createRecordBlueprint(
    closedCode,
    'Rail closed products',
    `attributes:
  - code: title
    value_type: string
  - code: categories
    value_type: relationship
    target_blueprint: '${categoryCode}'`,
    { views: 'views:\n  explorer:\n    type: explorer\n    browse_by: false' },
  );
  await page.goto(`/?blueprint=${closedCode}`);
  await expect(
    page.getByRole('button', { name: 'Show browse panel' }),
  ).toBeVisible();
  await expect(rail).toBeHidden();
});

test('opens the browse rail to a selection past the first page', async ({
  page,
}) => {
  const categoryCode = `rail_paged_category_${suffix()}`;
  const category = await createRecordBlueprint(
    categoryCode,
    'Paged rail categories',
    `attributes:
  - code: title
    value_type: string
  - code: parent
    value_type: relationship
    target_blueprint: '${categoryCode}'
    tree: true
    context_editable: default`,
  );
  const parent = await createRecord(category, [scalar('title', 'Catalog')]);
  // One more child than a page holds. Pages follow ID order, so the highest
  // ID always lands on page two. Batches keep fixture logins under the limit.
  const context = await defaultContext();
  const children = Array.from({ length: 101 }, (_, index) => ({
    id: crypto.randomUUID(),
    title: `Child ${String(index).padStart(3, '0')}`,
  }));
  for (let start = 0; start < children.length; start += 50)
    await request('/v1/records/batch', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        operations: children.slice(start, start + 50).map((child) => ({
          op: 'create',
          record_id: child.id,
          blueprint: {
            code: category.blueprint.code,
            version: category.blueprint.version,
          },
          values: [
            scalar('title', child.title),
            relationship('parent', parent.id),
          ].map((value) => ({ ...value, context_id: context.id })),
        })),
      }),
    });
  const lastTitle = children.reduce((highest, child) =>
    child.id > highest.id ? child : highest,
  ).title;

  await page.goto(`/?blueprint=${categoryCode}`);
  const rail = page.getByRole('complementary', {
    name: 'Browse by Paged rail categories',
  });
  await rail.getByRole('button', { name: 'Expand Catalog' }).click();
  await rail.getByRole('button', { name: 'Load more' }).click();
  await rail.getByRole('button', { name: new RegExp(`^${lastTitle}`) }).click();
  await expect(page.getByText('1 result')).toBeVisible();

  await page.reload();
  const selected = rail.getByRole('button', {
    name: new RegExp(`^${lastTitle}`),
  });
  await expect(selected).toHaveAttribute('aria-current', 'true');
  await expect(rail.getByRole('button', { name: 'Load more' })).toBeVisible();
  await rail.getByRole('button', { name: 'Load more' }).click();
  await expect(rail.getByRole('button', { name: 'Load more' })).toBeHidden();
  await expect(rail.getByText(lastTitle, { exact: true })).toHaveCount(1);
});

test("opens the browse rail on the blueprint's browse_by field", async ({
  page,
}) => {
  const brandCode = `rail_brand_${suffix()}`;
  const productCode = `rail_brand_product_${suffix()}`;
  const brand = await createRecordBlueprint(
    brandCode,
    'Rail brands',
    `attributes:
  - code: title
    value_type: string`,
  );
  const acme = await createRecord(brand, [scalar('title', 'Acme')]);
  const product = await createRecordBlueprint(
    productCode,
    'Rail brand products',
    `attributes:
  - code: title
    value_type: string
  - code: brand
    value_type: relationship
    target_blueprint: '${brandCode}'
    cardinality: one`,
    { views: 'views:\n  explorer:\n    type: explorer\n    browse_by: brand' },
  );
  await createRecord(product, [
    scalar('title', 'Anvil'),
    relationship('brand', acme.id),
  ]);

  // A flat target opens only because the blueprint names it.
  await page.goto(`/?blueprint=${productCode}`);
  const rail = page.getByRole('complementary', { name: 'Browse by brand' });
  await expect(rail.getByRole('button', { name: /^Acme/ })).toBeVisible();
});

test('filters explorer results with a one-level relationship facet', async ({
  page,
}) => {
  const colorCode = `facet_color_${suffix()}`;
  const productCode = `facet_color_product_${suffix()}`;
  const color = await createRecordBlueprint(
    colorCode,
    'Facet colors',
    `attributes:
  - code: title
    value_type: string`,
  );
  const red = await createRecord(color, [scalar('title', 'Red')]);
  const blue = await createRecord(color, [scalar('title', 'Blue')]);
  const product = await createRecordBlueprint(
    productCode,
    'Color products',
    `attributes:
  - code: title
    value_type: string
    tags: [searchable]
  - code: color
    value_type: relationship
    target_blueprint: '${colorCode}'
    cardinality: one`,
  );
  await createRecord(product, [
    scalar('title', 'Red shirt'),
    relationship('color', red.id),
  ]);
  await createRecord(product, [
    scalar('title', 'Blue shirt'),
    relationship('color', blue.id),
  ]);

  await page.goto(`/?blueprint=${productCode}`);
  await addRelationshipFilter(page, 'color', 'Red');

  await expect
    .poll(() => new URL(page.url()).searchParams.get('relationshipFacets'))
    .toContain(red.id);
  await expect(page.getByText('1 result')).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Red shirt', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Blue shirt', exact: true }),
  ).toBeHidden();
});

test('intersects selections from multiple relationship facets', async ({
  page,
}) => {
  const colorCode = `multi_color_${suffix()}`;
  const sizeCode = `multi_size_${suffix()}`;
  const productCode = `multi_product_${suffix()}`;
  const color = await createRecordBlueprint(
    colorCode,
    'Colors',
    'attributes:\n  - code: title\n    value_type: string',
  );
  const size = await createRecordBlueprint(
    sizeCode,
    'Sizes',
    'attributes:\n  - code: title\n    value_type: string',
  );
  const red = await createRecord(color, [scalar('title', 'Red')]);
  const blue = await createRecord(color, [scalar('title', 'Blue')]);
  const small = await createRecord(size, [scalar('title', 'Small')]);
  const large = await createRecord(size, [scalar('title', 'Large')]);
  const product = await createRecordBlueprint(
    productCode,
    'Products with multiple facets',
    `attributes:
  - code: title
    value_type: string
    tags: [searchable]
  - code: color
    value_type: relationship
    target_blueprint: '${colorCode}'
  - code: size
    value_type: relationship
    target_blueprint: '${sizeCode}'`,
  );
  await createRecord(product, [
    scalar('title', 'Red large shirt'),
    relationship('color', red.id),
    relationship('size', large.id),
  ]);
  await createRecord(product, [
    scalar('title', 'Red small shirt'),
    relationship('color', red.id),
    relationship('size', small.id),
  ]);
  await createRecord(product, [
    scalar('title', 'Blue large shirt'),
    relationship('color', blue.id),
    relationship('size', large.id),
  ]);

  await page.goto(`/?blueprint=${productCode}`);
  await addRelationshipFilter(page, 'color', 'Red');
  await addRelationshipFilter(page, 'size', 'Large');

  await expect(page.getByText('1 result')).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Red large shirt', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('cell', { name: 'Red small shirt', exact: true }),
  ).toBeHidden();
  await expect(
    page.getByRole('cell', { name: 'Blue large shirt', exact: true }),
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
  const blueprint = await createRecordBlueprint(
    code,
    'Paginated products',
    'attributes:\n  - code: title\n    value_type: string\n    tags: [searchable]',
  );
  for (let index = 0; index < 52; index += 1) {
    await createRecord(blueprint, [
      scalar('title', `Pagination product ${index}`),
    ]);
  }

  let searchRequests = 0;
  await page.route('**/api/v1/records/search', async (route) => {
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

test('searches a requested blueprint version and identifies outdated records', async ({
  page,
}) => {
  const code = `versions_${suffix()}`;
  const firstDefinition = `format_version: 1
code: '${code}'
name: Versioned products
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
    tags: [searchable]`;
  const first = await createRecordBlueprint(
    code,
    'Versioned products',
    'attributes:\n  - code: title\n    value_type: string\n    tags: [searchable]',
  );
  const oldRecord = await createRecord(first, [scalar('title', 'Old product')]);
  const second = await createRevision(
    first.blueprint.id,
    `${firstDefinition}
  - code: sku
    value_type: string`,
  );
  await publishRevision(second);

  await page.goto(`/?blueprint=${code}`);
  await expect(
    page.getByText(
      'Showing current-version records. 1 older-version record is hidden.',
    ),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Show all versions' }),
  ).toBeVisible();
  await expect(
    page.getByRole('link', { name: 'Review migrations' }),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: `View record ID ${oldRecord.id}` }),
  ).toBeHidden();

  await page.getByRole('button', { name: 'Show all versions' }).click();
  await expect(page).toHaveURL(/allVersions=true/);
  await expect(
    page.getByRole('button', { name: `View record ID ${oldRecord.id}` }),
  ).toBeVisible();

  await page.getByRole('button', { name: /^Search scope/ }).click();
  await page.getByRole('combobox', { name: 'Version scope' }).click();
  await page.getByRole('option', { name: 'Version 1' }).click();
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`blueprint=${code}.*version=1`));
  await expect(
    page.getByRole('cell', { name: 'v1 · Outdated', exact: true }),
  ).toBeVisible();
});
