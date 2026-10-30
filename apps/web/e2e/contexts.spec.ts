import { expect, test } from '@playwright/test';
import {
  fieldByLabel,
  commitField,
  createContext,
  createRecord,
  createRecordBlueprint,
  defaultContext,
  scalar,
  suffix,
} from './helpers';

test('resolves inherited values and saves a context-specific override', async ({
  page,
}) => {
  const code = `contextual_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Contextual product',
    'attributes:\n  - code: title\n    value_type: string\n  - code: stock\n    value_type: integer\n    context_editable: default',
  );
  const record = await createRecord(blueprint, [
    scalar('title', 'Default title'),
    scalar('stock', 5),
  ]);
  const context = await createContext(
    `uk_${suffix()}`,
    (await defaultContext()).id,
  );

  await page.goto(`/records/${record.id}`);
  await page.getByRole('tab', { name: context.code }).click();
  await expect(
    page.getByText('Inherited from default context').first(),
  ).toBeVisible();
  await expect(
    page.getByText('Inherited from default context: Default title'),
  ).toBeVisible();

  await expect(page.getByRole('tab', { name: context.code })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  // Only the default context may change stock, so it is shown read-only.
  await expect(page.getByRole('textbox', { name: 'stock' })).toHaveCount(0);
  const title = fieldByLabel(page, 'title');
  await title.fill('UK title');
  await commitField(page, record.id, title);

  await page.reload();
  await page.getByRole('tab', { name: context.code }).click();
  await expect(fieldByLabel(page, 'title')).toHaveValue('UK title');
  await page.getByRole('tab', { name: 'Default' }).click();
  await expect(fieldByLabel(page, 'title')).toHaveValue('Default title');
});
