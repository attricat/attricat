import { expect, test } from '@playwright/test';
import {
  createEntity,
  createEntityBlueprint,
  createRevision,
  publishRevision,
  scalar,
  suffix,
} from './helpers';

test('upgrades an outdated entity to the current blueprint revision', async ({
  page,
}) => {
  const code = `migrate_${suffix()}`;
  const first = await createEntityBlueprint(
    code,
    'Migrated product',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  const entity = await createEntity(first, [
    scalar('title', 'Existing product'),
  ]);
  const second = await createRevision(
    first.blueprint.id,
    `format_version = 1
code = "${code}"
name = "Migrated product"
kind = "entity"

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

  await page.goto(`/entities/${entity.id}`);
  await expect(page.getByText('Schema is outdated')).toBeVisible();
  await page.getByRole('link', { name: 'Upgrade blueprint' }).click();
  await expect(page.getByText('Upgrade from v1 to v2')).toBeVisible();
  await page.getByRole('button', { name: 'Upgrade entity' }).click();
  await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));
  await expect(page.getByText('Matches current schema')).toBeVisible();
  await expect(page.getByText('Existing product')).toBeVisible();
});
