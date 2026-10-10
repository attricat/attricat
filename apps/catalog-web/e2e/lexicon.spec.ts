import { expect, test } from '@playwright/test';
import { createRecordBlueprint, suffix } from './helpers';

test('translates a blueprint name reference and falls back after deleting it', async ({
  page,
}) => {
  const key = `gadget_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    `lexicon_${suffix()}`,
    `{{${key}}}`,
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  const blueprintUrl = `/manage/blueprints/${blueprint.blueprint.id}`;

  await page.goto(blueprintUrl);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(key);

  await page.goto('/manage/lexicon');
  await expect(
    page.getByRole('listitem').filter({ hasText: key }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Add translation' }).click();
  const dialog = page.getByRole('dialog', { name: 'Add translation' });
  await dialog.getByLabel('Key').fill(key);
  await dialog.getByLabel('Translation').fill('Lexicon gadget');
  await dialog.getByRole('button', { name: 'Save translation' }).click();
  await expect(dialog).toBeHidden();
  const entry = page.getByRole('row').filter({ hasText: key });
  await expect(entry).toContainText('Lexicon gadget');

  await page.goto(blueprintUrl);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(
    'Lexicon gadget',
  );

  await page.goto('/manage/lexicon');
  await entry
    .getByRole('button', { name: `Delete translation of ${key}` })
    .click();
  await page
    .getByRole('dialog')
    .getByRole('button', { name: 'Delete', exact: true })
    .click();
  await expect(entry).toBeHidden();
  await page.goto(blueprintUrl);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(key);
});
