import { expect, test } from '@playwright/test';
import {
  createEntity,
  createEntityBlueprint,
  scalar,
  suffix,
} from './helpers';

test('searches an entity and opens its preview', async ({ page }) => {
  const code = `product_search_${suffix()}`;
  const title = 'Red shirt';
  const blueprint = await createEntityBlueprint(
    code,
    'Search products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
  );
  const entity = await createEntity(blueprint, [scalar('title', title)]);

  await page.goto('/');
  await page.getByLabel('Select a Blueprint').click();
  await page.getByRole('option', { name: `Search products (${code})` }).click();
  await page.getByLabel('Query').fill('red');
  await page.getByRole('button', { name: 'Search' }).click();

  await expect(page).toHaveURL(new RegExp(`blueprint=${code}.*query=red`));
  await expect(page.getByText('1 result')).toBeVisible();
  await expect(page.getByRole('link', { name: entity.id })).toBeVisible();
  await page.getByRole('link', { name: entity.id }).click();
  await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));
  await expect(page.getByText(title)).toBeVisible();
});

test('creates an entity from a blueprint', async ({ page }) => {
  const code = `product_create_${suffix()}`;
  const title = 'Created in browser';
  await createEntityBlueprint(
    code,
    'Create products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
  );

  await page.goto('/entities/new');
  await page.getByLabel('Blueprint').click();
  await page.getByRole('option', { name: `Create products (${code})` }).click();
  await page.getByRole('button', { name: 'Load blueprint' }).click();
  await page.getByLabel('title').fill(title);
  await page.getByRole('button', { name: 'Create entity' }).click();

  await expect(page).toHaveURL(/\/entities\/[0-9a-f-]{36}$/);
  await expect(page.getByText(title)).toBeVisible();
});

test('creates an entity with typed scalar values', async ({ page }) => {
  const code = `typed_create_${suffix()}`;
  await createEntityBlueprint(
    code,
    'Typed products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\n\n[[attributes]]\ncode = "price"\nvalue_type = "number"\n\n[[attributes]]\ncode = "quantity"\nvalue_type = "integer"\n\n[[attributes]]\ncode = "available"\nvalue_type = "boolean"\n\n[[attributes]]\ncode = "launch_date"\nvalue_type = "date"\n\n[[attributes]]\ncode = "opening_time"\nvalue_type = "time"',
  );

  await page.goto('/entities/new');
  await page.getByLabel('Blueprint').click();
  await page.getByRole('option', { name: `Typed products (${code})` }).click();
  await page.getByRole('button', { name: 'Load blueprint' }).click();
  await page.getByLabel('title').fill('Typed product');
  await page.getByLabel('price').fill('19.95');
  await page.getByLabel('quantity').fill('4');
  await page.getByLabel('available').click();
  await page.getByRole('option', { name: 'True' }).click();
  await page.getByLabel('launch_date').fill('2026-08-20');
  await page.getByLabel('opening_time').fill('09:30:00 America/New_York');
  await page.getByRole('button', { name: 'Create entity' }).click();

  await expect(page).toHaveURL(/\/entities\/[0-9a-f-]{36}$/);
  await expect(page.getByText('Typed product')).toBeVisible();
  await expect(page.getByText('19.95')).toBeVisible();
  await expect(page.getByText('4', { exact: true })).toBeVisible();
  await expect(page.getByText('Yes')).toBeVisible();
});

test('rejects a browser create that violates a blueprint schema', async ({
  page,
}) => {
  const code = `product_schema_${suffix()}`;
  await createEntityBlueprint(
    code,
    'Schema products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
    {
      entitySchema:
        '{"type":"object","required":["title"],"properties":{"title":{"minLength":3}}}',
    },
  );

  await page.goto('/entities/new');
  await page.getByLabel('Blueprint').click();
  await page.getByRole('option', { name: `Schema products (${code})` }).click();
  await page.getByRole('button', { name: 'Load blueprint' }).click();
  await page.getByLabel('title').fill('no');
  await page.getByRole('button', { name: 'Create entity' }).click();

  await expect(page).toHaveURL(/\/entities\/new$/);
  await expect(page.getByText('Request failed (422)')).toBeVisible();

  await page.getByLabel('title').fill('Valid title');
  await page.getByRole('button', { name: 'Create entity' }).click();
  await expect(page).toHaveURL(/\/entities\/[0-9a-f-]{36}$/);
});

test('creates a context from context management', async ({ page }) => {
  const code = `market_${suffix()}`;

  await page.goto('/');
  await page.getByRole('link', { name: 'Manage contexts' }).click();
  await expect(page).toHaveURL(/\/contexts$/);
  await page.getByRole('link', { name: 'Create context' }).click();
  await page.getByLabel('Code').fill(code);
  await page.getByLabel('Parent context').click();
  await page.getByRole('option', { name: 'default' }).click();
  await page.getByLabel('Metadata').fill('{"market":"US"}');
  await page.getByRole('button', { name: 'Create context' }).click();

  await expect(page).toHaveURL(/\/contexts$/);
  await expect(page.getByText(code)).toBeVisible();
  await expect(page.getByText('{"market":"US"}')).toBeVisible();
});

test('edits scalar values and replaces a typed relationship', async ({
  page,
}) => {
  const categoryCode = `category_edit_${suffix()}`;
  const productCode = `product_edit_${suffix()}`;
  const category = await createEntityBlueprint(
    categoryCode,
    'Edit categories',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
  );
  await createEntity(category, [scalar('title', 'Sale')]);
  const product = await createEntityBlueprint(
    productCode,
    'Edit products',
    `[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]\n\n[[attributes]]\ncode = "categories"\nvalue_type = "relationship"\ntarget_blueprint = "${categoryCode}"`,
  );
  const entity = await createEntity(product, [scalar('title', 'Before edit')]);

  await page.goto(`/entities/${entity.id}`);
  await page.getByRole('link', { name: 'Edit entity' }).click();
  await expect(page.getByLabel('Context')).toHaveText('default');
  await page.getByLabel('title').fill('After edit');
  await page.getByLabel('categories').click();
  await page.getByRole('option', { name: 'Sale' }).click();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Save changes' }).click();

  await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));
  await expect(page.getByText('After edit')).toBeVisible();
  await expect(page.getByText('Sale')).toBeVisible();
});
