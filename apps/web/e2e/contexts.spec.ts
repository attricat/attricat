import { expect, test } from '@playwright/test';
import {
  fieldByLabel,
  commitField,
  createContext,
  createRecord,
  createRecordBlueprint,
  defaultContext,
  recordSave,
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

test('switches context with one click on a tab that saves the last edit', async ({
  page,
}) => {
  const blueprint = await createRecordBlueprint(
    `context_switch_${suffix()}`,
    'Context switch product',
    'attributes:\n  - code: title\n    value_type: string',
  );
  const record = await createRecord(blueprint, [scalar('title', 'Default')]);
  const parentId = (await defaultContext()).id;
  const first = await createContext(`first_${suffix()}`, parentId);
  const second = await createContext(`second_${suffix()}`, parentId);

  await page.goto(`/records/${record.id}`);
  await page.getByRole('tab', { name: first.code }).click();
  await fieldByLabel(page, 'title').fill('First title');
  // Leaving the field for another context commits it; the tab waits for
  // that save instead of ignoring the click.
  const saved = recordSave(page, record.id);
  await page.getByRole('tab', { name: second.code }).click();
  expect((await saved).ok()).toBe(true);
  await expect(page.getByRole('tab', { name: second.code })).toHaveAttribute(
    'aria-selected',
    'true',
  );

  await page.reload();
  await page.getByRole('tab', { name: first.code }).click();
  await expect(fieldByLabel(page, 'title')).toHaveValue('First title');
});
