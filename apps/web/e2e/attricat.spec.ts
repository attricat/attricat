import { expect, test } from '@playwright/test';
import {
  commitField,
  createRecord,
  createRecordBlueprint,
  recordSave,
  relationship,
  scalar,
  signInAsMember,
  suffix,
} from './helpers';

test('searches a record and opens its preview', async ({ page }) => {
  const code = `product_search_${suffix()}`;
  const title = 'Red shirt';
  const blueprint = await createRecordBlueprint(
    code,
    'Search products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
  );
  const record = await createRecord(blueprint, [scalar('title', title)]);

  await page.goto('/');
  await page.getByLabel('Select a Blueprint').click();
  await page.getByRole('option', { name: `Search products (${code})` }).click();
  await page.getByLabel('Query').fill('red');
  await page.getByRole('button', { name: 'Search', exact: true }).click();

  await expect(page).toHaveURL(new RegExp(`blueprint=${code}.*query=red`));
  await expect(page.getByText('1 result')).toBeVisible();
  await expect(
    page.getByRole('button', { name: `View record ID ${record.id}` }),
  ).toBeVisible();
  await page
    .getByRole('button', { name: `Record actions for ${record.id}` })
    .click();
  await page.getByRole('menuitem', { name: 'Search info' }).click();
  await expect(page.getByRole('dialog', { name: 'Search info' })).toContainText(
    'red in title',
  );
  await page.keyboard.press('Escape');
  // On desktop a result opens in the record panel beside the results.
  await page.getByRole('link', { name: title }).click();
  await expect(page).toHaveURL(new RegExp(`record=${record.id}`));
  await expect(page.getByLabel('title')).toHaveValue(title);
  await page.getByRole('link', { name: 'Open full page' }).click();
  await expect(page).toHaveURL(new RegExp(`/records/${record.id}$`));
  await expect(page.getByLabel('title')).toHaveValue(title);
});

test('closes the record panel on a click outside it', async ({ page }) => {
  const code = `product_panel_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Panel products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  const first = await createRecord(blueprint, [scalar('title', 'First panel')]);
  const second = await createRecord(blueprint, [
    scalar('title', 'Second panel'),
  ]);

  await page.goto(`/?blueprint=${code}`);
  await page.getByRole('link', { name: 'First panel' }).click();
  await expect(page).toHaveURL(new RegExp(`record=${first.id}`));
  // Another result switches the panel instead of closing it.
  await page.getByRole('link', { name: 'Second panel' }).click();
  await expect(page).toHaveURL(new RegExp(`record=${second.id}`));
  // Dialogs opened from the panel count as inside it.
  const panel = page.getByRole('complementary', {
    name: 'Panel products record',
  });
  await panel.getByRole('button', { name: 'Actions', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Duplicate record' }).click();
  await page
    .getByRole('dialog')
    .getByRole('button', { name: 'Cancel' })
    .click();
  await expect(page).toHaveURL(new RegExp(`record=${second.id}`));

  await page.getByText(/^2 results$/).click();
  await expect(page).not.toHaveURL(/record=/);
  await expect(panel).toHaveCount(0);
});

test('keeps the record panel open while choosing from a select in it', async ({
  page,
}) => {
  const code = `product_panel_select_${suffix()}`;
  const status = JSON.stringify({
    type: 'string',
    enum: ['draft', 'live'],
    'x-attricat-status': {
      version: 1,
      options: [
        { code: 'draft', label: 'Draft' },
        { code: 'live', label: 'Live' },
      ],
      transitions: [
        { from: null, to: 'draft' },
        { from: 'draft', to: 'live' },
      ],
    },
  });
  const blueprint = await createRecordBlueprint(
    code,
    'Panel select products',
    `[[attributes]]\ncode = "title"\nvalue_type = "string"\n\n[[attributes]]\ncode = "status"\nvalue_type = "string"\nvalue_schema = '${status}'`,
  );
  const record = await createRecord(blueprint, [
    scalar('title', 'Selectable'),
    scalar('status', 'draft'),
  ]);

  await page.goto(`/?blueprint=${code}`);
  await page.getByRole('link', { name: 'Selectable' }).click();
  const panel = page.getByRole('complementary', {
    name: 'Panel select products record',
  });
  const select = panel.getByRole('combobox', { name: 'status', exact: true });
  await select.click();
  const saved = recordSave(page, record.id);
  await page.getByRole('option', { name: 'Live', exact: true }).click();
  expect((await saved).ok()).toBe(true);
  await expect(panel).toBeVisible();
  await expect(select).toContainText('Live');
  await expect(page).toHaveURL(new RegExp(`record=${record.id}`));
});

test('restores the last selected blueprint and prioritizes the URL', async ({
  page,
}) => {
  const storedCode = `product_stored_${suffix()}`;
  const urlCode = `product_url_${suffix()}`;
  const attributes = '[[attributes]]\ncode = "title"\nvalue_type = "string"';
  await createRecordBlueprint(storedCode, 'Stored products', attributes);
  await createRecordBlueprint(urlCode, 'URL products', attributes);

  await page.goto('/');
  await page.evaluate((code) => {
    sessionStorage.setItem('attricat.explorer.last-blueprint', code);
  }, storedCode);
  await page.goto(`/?blueprint=${urlCode}`);

  const blueprintSelect = page.getByLabel('Select a Blueprint').first();
  await expect(blueprintSelect).toContainText(urlCode);

  await page.goto('/');
  await expect(blueprintSelect).toContainText(urlCode);
  await page.reload();
  await expect(blueprintSelect).toContainText(urlCode);
});

test('creates a record from a blueprint', async ({ page }) => {
  const code = `product_create_${suffix()}`;
  const title = 'Created in browser';
  await createRecordBlueprint(
    code,
    'Create products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
  );

  await page.goto('/records/new');
  await page.getByLabel('Blueprint').click();
  await page.getByRole('option', { name: `Create products (${code})` }).click();
  await page.getByRole('button', { name: 'Load blueprint' }).click();
  await page.getByLabel('title').fill(title);
  await page.getByRole('button', { name: 'Create record' }).click();

  await expect(page).toHaveURL(/\/records\/[0-9a-f-]{36}$/);
  // The record page shows each saved value in its inline editor.
  await expect(page.getByLabel('title')).toHaveValue(title);
});

test('creates a record with typed scalar values', async ({ page }) => {
  const code = `typed_create_${suffix()}`;
  await createRecordBlueprint(
    code,
    'Typed products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\n\n[[attributes]]\ncode = "price"\nvalue_type = "number"\n\n[[attributes]]\ncode = "quantity"\nvalue_type = "integer"\n\n[[attributes]]\ncode = "available"\nvalue_type = "boolean"\n\n[[attributes]]\ncode = "launch_date"\nvalue_type = "date"\n\n[[attributes]]\ncode = "opening_time"\nvalue_type = "time"',
  );

  await page.goto('/records/new');
  await page.getByLabel('Blueprint').click();
  await page.getByRole('option', { name: `Typed products (${code})` }).click();
  await page.getByRole('button', { name: 'Load blueprint' }).click();
  await page.getByLabel('title').fill('Typed product');
  await page.getByLabel('price').fill('19.95');
  await page.getByLabel('quantity').fill('4');
  await page.getByLabel('available').click();
  await page.getByRole('option', { name: 'True' }).click();
  await page.getByLabel('Launch date').fill('2026-08-20');
  await page.getByLabel('Opening time').fill('09:30:00 America/New_York');
  await page.getByRole('button', { name: 'Create record' }).click();

  await expect(page).toHaveURL(/\/records\/[0-9a-f-]{36}$/);
  // The record page shows each saved value in its inline editor.
  await expect(page.getByLabel('title')).toHaveValue('Typed product');
  await expect(page.getByLabel('price')).toHaveValue('19.95');
  await expect(page.getByLabel('quantity')).toHaveValue('4');
  await expect(page.getByLabel('available')).toHaveText('True');
  await expect(page.getByLabel('Launch date')).toHaveValue('2026-08-20');
});

test('renders a record heading component from its detail view', async ({
  page,
}) => {
  const code = `heading_product_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Heading products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
    {
      views:
        '[views.detail]\ntype = "stack"\ncomponent = { id = "attricat.record_heading", version = 1 }\nchildren = [{ type = "field", field = "title" }, { type = "text", text = "Generated catalog product" }]',
    },
  );
  const record = await createRecord(blueprint, [
    scalar('title', 'Product 001'),
  ]);

  await page.goto(`/records/${record.id}`);

  await expect(
    page.getByRole('heading', { name: 'Product 001' }),
  ).toBeVisible();
  await expect(page.getByText('Generated catalog product')).toBeVisible();
});

test('rejects a browser create that violates a blueprint schema', async ({
  page,
}) => {
  let createRequests = 0;
  page.on('request', (request) => {
    if (
      request.method() === 'POST' &&
      new URL(request.url()).pathname === '/api/v1/records'
    )
      createRequests += 1;
  });
  const code = `product_schema_${suffix()}`;
  await createRecordBlueprint(
    code,
    'Schema products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
    {
      recordSchema:
        '{"type":"object","required":["title"],"properties":{"title":{"minLength":3}}}',
    },
  );

  await page.goto('/records/new');
  await page.getByLabel('Blueprint').click();
  await page.getByRole('option', { name: `Schema products (${code})` }).click();
  await page.getByRole('button', { name: 'Load blueprint' }).click();
  await page.getByLabel('title').fill('no');

  await expect(
    page.getByText('Does not meet the schema requirements.'),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Create record' }).click();
  await expect(page).toHaveURL(/\/records\/new(\?|$)/);
  expect(createRequests).toBe(0);

  await page.getByLabel('title').fill('Valid title');
  await page.getByRole('button', { name: 'Create record' }).click();
  await expect(page).toHaveURL(/\/records\/[0-9a-f-]{36}$/);
});

test('creates a context from context management', async ({ page }) => {
  const code = `market_${suffix()}`;

  await page.goto('/manage/contexts');
  await expect(page).toHaveURL(/\/manage\/contexts$/);
  await page.getByRole('link', { name: 'Create context' }).click();
  await page.getByLabel('Code').fill(code);
  await page.getByLabel('Parent context').click();
  await page.getByRole('option', { name: 'default' }).click();
  await page.getByLabel('Metadata').fill('{"market":"US"}');
  await page.getByRole('button', { name: 'Create context' }).click();

  await expect(page).toHaveURL(/\/manage\/contexts$/);
  await expect(page.getByText(code)).toBeVisible();
  await expect(page.getByText('{"market":"US"}')).toBeVisible();
});

test('navigates outgoing, hierarchical, and incoming relationships', async ({
  browser,
}) => {
  const categoryCode = `category_views_${suffix()}`;
  const productCode = `product_views_${suffix()}`;
  const category = await createRecordBlueprint(
    categoryCode,
    'Relationship categories',
    `[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "parent"
value_type = "relationship"
target_blueprint = "${categoryCode}"`,
    {
      views: `[views.detail]
type = "stack"
children = [{ type = "relationship_list", field = "parent", component = { id = "attricat.relationship_hierarchy", version = 1 } }, { type = "field", field = "title" }, { type = "incoming_relationship_list", label = "Products in category", page_size = 10, relationships = [{ source_blueprint = "${productCode}", field = "category" }], component = { id = "attricat.incoming_relationship_list_display", version = 1 } }]`,
    },
  );
  const parent = await createRecord(category, [scalar('title', 'Departments')]);
  const child = await createRecord(category, [
    scalar('title', 'Shoes'),
    relationship('parent', parent.id),
  ]);
  const product = await createRecordBlueprint(
    productCode,
    'Relationship products',
    `[[attributes]]
code = "title"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "${categoryCode}"`,
    {
      views:
        '[views.detail]\ntype = "stack"\nchildren = [{ type = "field", field = "title" }, { type = "relationship_list", field = "category", component = { id = "attricat.relationship_hierarchy", version = 1, props = { parent_field = "parent" } } }]',
    },
  );
  const item = await createRecord(product, [
    scalar('title', 'Running shoe'),
    relationship('category', child.id),
  ]);

  // Writers edit relationships in place; a viewer sees the display links.
  const page = await signInAsMember(browser, 'viewer');
  await page.goto(`/records/${item.id}`);
  const hierarchy = page.getByLabel('Hierarchy');
  await expect(
    hierarchy.getByRole('link', { name: 'Departments' }),
  ).toBeVisible();
  await expect(hierarchy.getByRole('link', { name: 'Shoes' })).toBeVisible();
  await hierarchy.getByRole('link', { name: 'Shoes' }).click();
  await expect(page).toHaveURL(new RegExp(`/records/${child.id}$`));

  await page.getByRole('button', { name: 'Products in category' }).click();
  const dialog = page.getByRole('dialog', { name: 'Products in category' });
  await expect(
    dialog.getByRole('link', { name: 'Running shoe' }),
  ).toBeVisible();
  await dialog.getByRole('link', { name: 'Running shoe' }).click();
  await expect(page).toHaveURL(new RegExp(`/records/${item.id}$`));
  await page.context().close();
});

test('edits scalar values and replaces a typed relationship', async ({
  page,
}) => {
  const categoryCode = `category_edit_${suffix()}`;
  const productCode = `product_edit_${suffix()}`;
  const category = await createRecordBlueprint(
    categoryCode,
    'Edit categories',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
  );
  await createRecord(category, [scalar('title', 'Sale')]);
  const product = await createRecordBlueprint(
    productCode,
    'Edit products',
    `[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]\n\n[[attributes]]\ncode = "categories"\nvalue_type = "relationship"\ntarget_blueprint = "${categoryCode}"`,
  );
  const record = await createRecord(product, [scalar('title', 'Before edit')]);

  await page.goto(`/records/${record.id}`);
  await expect(page.getByRole('tab', { name: 'Default' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  const title = page.getByLabel('title');
  await title.fill('After edit');
  await commitField(page, record.id, title, 'Enter');
  // A relationship choice saves as soon as it is applied.
  const relationshipSaved = recordSave(page, record.id);
  await page.getByRole('button', { name: 'Choose categories' }).click();
  await page.getByRole('button', { name: 'Select Sale' }).click();
  await page.getByRole('button', { name: 'Apply' }).click();
  expect((await relationshipSaved).ok()).toBe(true);

  await page.reload();
  await expect(page.getByLabel('title')).toHaveValue('After edit');
  await expect(page.getByText('Sale')).toBeVisible();
});
