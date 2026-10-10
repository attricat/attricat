import { expect, test } from '@playwright/test';
import {
  createRecord,
  createRecordBlueprint,
  createRevision,
  publishRevision,
  scalar,
  suffix,
} from './helpers';

test('upgrades an outdated record to the current blueprint revision', async ({
  page,
}) => {
  const code = `migrate_${suffix()}`;
  const first = await createRecordBlueprint(
    code,
    'Migrated product',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  const record = await createRecord(first, [
    scalar('title', 'Existing product'),
  ]);
  const second = await createRevision(
    first.blueprint.id,
    `format_version = 1
code = "${code}"
name = "Migrated product"
kind = "record"
record_schema = '{"type":"object","required":["description"]}'

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "description"
value_type = "string"`,
  );
  await publishRevision(second);

  await page.goto(`/records/${record.id}`);
  await expect(page.getByLabel('Schema is outdated')).toBeVisible();
  await page.getByRole('button', { name: 'Actions' }).click();
  await page.getByRole('menuitem', { name: 'Upgrade blueprint' }).click();
  await expect(page.getByText('Upgrade from v1 to v2')).toBeVisible();
  await expect(page.getByLabel('description')).toBeVisible();
  await page.getByLabel('description').fill('Added during migration');
  await page.getByRole('button', { name: 'Upgrade record' }).click();
  await expect(page).toHaveURL(new RegExp(`/records/${record.id}$`));
  await expect(page.getByLabel('Matches current schema')).toBeVisible();
  // Writers see the values in the record page's editors.
  await expect(page.getByRole('textbox', { name: 'title' })).toHaveValue(
    'Existing product',
  );
  await expect(page.getByRole('textbox', { name: 'description' })).toHaveValue(
    'Added during migration',
  );
});
