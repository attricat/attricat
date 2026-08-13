import { expect, test } from '@playwright/test';

const apiUrl = 'http://127.0.0.1:43100';
const suffix = () => crypto.randomUUID().slice(0, 8);

const request = async <T>(path: string, init?: RequestInit): Promise<T> => {
  const response = await fetch(`${apiUrl}${path}`, init);
  if (!response.ok) {
    throw new Error(
      `E2E setup request failed: ${response.status} ${await response.text()}`,
    );
  }
  return response.json() as Promise<T>;
};

type Blueprint = { blueprint: { code: string; version: number } };
type Entity = { id: string };

const createBlueprint = async (
  code: string,
  name: string,
  attributes: string,
) =>
  request<Blueprint>('/blueprints', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      definition: `format_version = 1\ncode = "${code}"\nname = "${name}"\nkind = "entity"\n\n[display.dropdown_option]\nfields = ["title"]\n\n${attributes}`,
    }),
  });

const createEntity = async (blueprint: Blueprint, values: unknown[]) =>
  request<Entity>('/v1/entities', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      blueprint: {
        code: blueprint.blueprint.code,
        version: blueprint.blueprint.version,
      },
      values,
    }),
  });

const scalar = (attributeCode: string, value: string) => ({
  kind: 'scalar',
  attribute_code: attributeCode,
  value,
});

test('searches an entity and opens its preview', async ({ page }) => {
  const code = `product_search_${suffix()}`;
  const title = 'Red shirt';
  const blueprint = await createBlueprint(
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
  await page.getByRole('link', { name: entity.id }).click();
  await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));
  await expect(page.locator('pre')).toContainText(title);
});

test('creates an entity from a blueprint', async ({ page }) => {
  const code = `product_create_${suffix()}`;
  const title = 'Created in browser';
  await createBlueprint(
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
  await expect(page.locator('pre')).toContainText(title);
});

test('creates a context from context management', async ({ page }) => {
  const code = `market_${suffix()}`;

  await page.goto('/');
  await page.getByRole('link', { name: 'Manage contexts' }).click();
  await expect(page).toHaveURL(/\/contexts$/);
  await page.getByRole('link', { name: 'Create context' }).click();
  await page.getByLabel('Code').fill(code);
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
  const category = await createBlueprint(
    categoryCode,
    'Edit categories',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]',
  );
  const categoryEntity = await createEntity(category, [
    scalar('title', 'Sale'),
  ]);
  const product = await createBlueprint(
    productCode,
    'Edit products',
    `[[attributes]]\ncode = "title"\nvalue_type = "string"\ntags = ["searchable"]\n\n[[attributes]]\ncode = "categories"\nvalue_type = "relationship"\ntarget_blueprint = "${categoryCode}"`,
  );
  const entity = await createEntity(product, [scalar('title', 'Before edit')]);

  await page.goto(`/entities/${entity.id}`);
  await page.getByRole('link', { name: 'Edit entity' }).click();
  await expect(page.getByLabel('Context')).toHaveText('Default');
  await page.getByLabel('title').fill('After edit');
  await page.getByLabel('categories').click();
  await page.getByRole('option', { name: 'Sale' }).click();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Save changes' }).click();

  await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));
  await expect(page.locator('pre')).toContainText('After edit');
  await expect(page.locator('pre')).toContainText(categoryEntity.id);
});
