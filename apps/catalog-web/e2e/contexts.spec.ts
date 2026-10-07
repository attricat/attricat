import { expect, test } from '@playwright/test';
import {
  commitField,
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
  await page.getByRole('tab', { name: context.code }).click();
  await expect(
    page.getByText('Inherited from default context').first(),
  ).toBeVisible();
  await expect(page.getByText('Default title')).toBeVisible();

  await expect(page.getByRole('tab', { name: context.code })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  // Only the default context may change stock, so it is shown read-only.
  await expect(page.getByRole('textbox', { name: 'stock' })).toHaveCount(0);
  const title = page.getByLabel('title');
  await title.fill('UK title');
  await commitField(page, entity.id, title);

  await page.reload();
  await page.getByRole('tab', { name: context.code }).click();
  await expect(page.getByLabel('title')).toHaveValue('UK title');
  await page.getByRole('tab', { name: 'Default' }).click();
  await expect(page.getByLabel('title')).toHaveValue('Default title');
});
