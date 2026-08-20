import { expect, test } from '@playwright/test';
import {
  createContext,
  createEntity,
  createEntityBlueprint,
  defaultContext,
  scalar,
  suffix,
} from './helpers';

test('resolves inherited values and saves a context-specific override', async ({
  page,
}) => {
  const code = `contextual_${suffix()}`;
  const blueprint = await createEntityBlueprint(
    code,
    'Contextual product',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\n\n[[attributes]]\ncode = "stock"\nvalue_type = "integer"\ncontext_editable = "default"',
  );
  const entity = await createEntity(blueprint, [
    scalar('title', 'Default title'),
    scalar('stock', 5),
  ]);
  const context = await createContext(
    `uk_${suffix()}`,
    (await defaultContext()).id,
  );

  await page.goto(`/entities/${entity.id}`);
  await page.getByLabel('Context').click();
  await page.getByRole('option', { name: context.code }).click();
  await expect(page.getByText('Using default').first()).toBeVisible();
  await expect(page.getByText('Default title')).toBeVisible();

  await page.getByRole('link', { name: 'Edit entity' }).click();
  await page.getByLabel('Context').click();
  await page.getByRole('option', { name: context.code }).click();
  await expect(page.getByLabel('stock')).toBeDisabled();
  await page.getByLabel('title').fill('UK title');
  await page.getByRole('button', { name: 'Save changes' }).click();
  await page.getByLabel('Context').click();
  await page.getByRole('option', { name: context.code }).click();
  await expect(page.getByText('UK title')).toBeVisible();
});
