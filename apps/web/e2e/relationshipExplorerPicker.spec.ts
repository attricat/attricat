import { expect, test, type Page } from '@playwright/test';
import {
  createRecord,
  createRecordBlueprint,
  fieldByLabel,
  relationship,
  request,
  scalar,
  suffix,
} from './helpers';

type FormValue = { attribute_code: string; target_record_id?: string };

/** The records a relationship of a saved record links to. */
const linked = async (recordId: string, code: string) =>
  (await request<{ values: FormValue[] }>(`/v1/records/${recordId}`)).values
    .filter((value) => value.attribute_code === code)
    .map((value) => value.target_record_id)
    .sort();

const barName = 'Choose records for a relationship';

/**
 * Opens the Explorer from a relationship picker and returns its tab. The
 * picker stays open behind it, waiting for the selection.
 */
const openExplorerPicker = async (page: Page, field: string) => {
  await page.getByRole('button', { name: `Choose ${field}` }).click();
  const dialog = page.getByRole('dialog', { name: /^Select / });
  const [explorer] = await Promise.all([
    page.context().waitForEvent('page'),
    dialog.getByRole('link', { name: 'Browse' }).click(),
  ]);
  await expect(
    explorer.getByRole('complementary', { name: barName }),
  ).toBeVisible();
  return { dialog, explorer };
};

const selectRow = (explorer: Page, label: string) =>
  explorer.getByRole('checkbox', { name: `Select ${label}` }).check();

for (const colorScheme of ['light', 'dark'] as const) {
  test(`chooses relationship targets in the Explorer in ${colorScheme} mode`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme });
    const run = suffix();
    const category = await createRecordBlueprint(
      `picker_category_${run}`,
      `Picker categories ${run}`,
      'attributes:\n  - code: title\n    value_type: string\n    tags: [searchable]',
    );
    const ids: Record<string, string> = {};
    for (const title of ['Sale', 'Summer', 'Winter'])
      ids[title] = (await createRecord(category, [scalar('title', title)])).id;
    const product = await createRecordBlueprint(
      `picker_product_${run}`,
      `Picker products ${run}`,
      `attributes:
  - code: title
    value_type: string
  - code: categories
    value_type: relationship
    target_blueprint: picker_category_${run}
  - code: featured
    value_type: relationship
    target_blueprint: picker_category_${run}
    cardinality: one`,
    );
    const record = await createRecord(product, [scalar('title', 'Shirt')]);
    await page.goto(`/records/${record.id}`);

    // Many targets: search and select in the Explorer, then use them.
    const { dialog, explorer } = await openExplorerPicker(page, 'categories');
    await explorer.emulateMedia({ colorScheme });
    // Searching keeps the tab choosing for the picker.
    await explorer.getByLabel('Query').fill('S');
    await explorer.getByLabel('Query').press('Enter');
    await expect(explorer).toHaveURL(/relationshipPicker=/);
    await selectRow(explorer, 'Sale');
    await selectRow(explorer, 'Summer');
    // A picker tab only chooses records.
    await expect(
      explorer.getByRole('button', { name: 'Exit selection' }),
    ).toHaveCount(0);
    await Promise.all([
      explorer.waitForEvent('close'),
      explorer.getByRole('button', { name: 'Use 2 selected records' }).click(),
    ]);
    await expect(dialog.getByRole('checkbox', { name: 'Sale' })).toBeChecked();
    await expect(
      dialog.getByRole('checkbox', { name: 'Summer' }),
    ).toBeChecked();
    await expect(
      dialog.getByRole('checkbox', { name: 'Winter' }),
    ).not.toBeChecked();
    // The selection is a draft until it is applied.
    await dialog.getByRole('button', { name: 'Apply' }).click();
    await expect
      .poll(() => linked(record.id, 'categories'))
      .toEqual([ids.Sale, ids.Summer].sort());

    // One target: choosing another row replaces the first.
    const single = await openExplorerPicker(page, 'featured');
    await expect(
      single.explorer.getByRole('checkbox', {
        name: /^Select loaded records/,
      }),
    ).toHaveCount(0);
    await selectRow(single.explorer, 'Sale');
    await selectRow(single.explorer, 'Winter');
    await Promise.all([
      single.explorer.waitForEvent('close'),
      single.explorer
        .getByRole('button', { name: 'Use 1 selected record' })
        .click(),
    ]);
    await expect(
      single.dialog.getByRole('radio', { name: 'Winter' }),
    ).toBeChecked();
    await single.dialog.getByRole('button', { name: 'Apply' }).click();
    await expect
      .poll(() => linked(record.id, 'featured'))
      .toEqual([ids.Winter]);
  });
}

test('a scoped relationship opens the Explorer with its scope filter', async ({
  page,
}) => {
  const run = suffix();
  const group = await createRecordBlueprint(
    `picker_group_${run}`,
    `Picker groups ${run}`,
    'attributes:\n  - code: title\n    value_type: string\n    tags: [searchable]',
  );
  const tools = await createRecord(group, [scalar('title', 'Tools')]);
  const garden = await createRecord(group, [scalar('title', 'Garden')]);
  const option = await createRecordBlueprint(
    `picker_option_${run}`,
    `Picker options ${run}`,
    `attributes:
  - code: title
    value_type: string
    tags: [searchable]
  - code: group
    value_type: relationship
    target_blueprint: picker_group_${run}
    cardinality: one`,
  );
  await createRecord(option, [
    scalar('title', 'Hammer'),
    relationship('group', tools.id),
  ]);
  await createRecord(option, [
    scalar('title', 'Rake'),
    relationship('group', garden.id),
  ]);
  const item = await createRecordBlueprint(
    `picker_item_${run}`,
    `Picker items ${run}`,
    `attributes:
  - code: title
    value_type: string
  - code: group
    value_type: relationship
    target_blueprint: picker_group_${run}
    cardinality: one
  - code: option
    value_type: relationship
    target_blueprint: picker_option_${run}
    cardinality: one
    target_scope: {attribute: group, equals: group}`,
  );
  const record = await createRecord(item, [
    scalar('title', 'Kit'),
    relationship('group', tools.id),
  ]);
  await page.goto(`/records/${record.id}`);

  const { explorer } = await openExplorerPicker(page, 'option');
  await expect(
    explorer.getByRole('checkbox', { name: 'Select Hammer' }),
  ).toBeVisible();
  await expect(
    explorer.getByRole('checkbox', { name: 'Select Rake' }),
  ).toHaveCount(0);
  await explorer.getByRole('button', { name: 'Cancel' }).click();
});

test('creates relationship targets from the Explorer tab', async ({ page }) => {
  const run = suffix();
  const category = await createRecordBlueprint(
    `picker_new_category_${run}`,
    `New categories ${run}`,
    'attributes:\n  - code: title\n    value_type: string\n    tags: [searchable]',
  );
  const sale = await createRecord(category, [scalar('title', 'Sale')]);
  const product = await createRecordBlueprint(
    `picker_new_product_${run}`,
    `New products ${run}`,
    `attributes:
  - code: title
    value_type: string
  - code: categories
    value_type: relationship
    target_blueprint: picker_new_category_${run}`,
  );
  const record = await createRecord(product, [scalar('title', 'Shirt')]);
  await page.goto(`/records/${record.id}`);

  const { dialog, explorer } = await openExplorerPicker(page, 'categories');
  await selectRow(explorer, 'Sale');
  await explorer
    .getByRole('link', { name: `Create New categories ${run}` })
    .click();
  await expect(explorer).toHaveURL(/\/records\/new\?.*relationshipPicker=/);
  // Create and add another selects every record created for the picker.
  await fieldByLabel(explorer, 'title').fill('Autumn');
  await explorer
    .getByRole('button', { name: 'Create and add another' })
    .click();
  await expect(
    explorer.getByText('Record created and selected for the relationship.'),
  ).toBeVisible();
  await fieldByLabel(explorer, 'title').fill('Spring');
  await explorer.getByRole('button', { name: 'Create record' }).click();

  // Back in the Explorer, the earlier selection and the new records wait.
  await expect(
    explorer.getByRole('complementary', { name: barName }),
  ).toBeVisible();
  await Promise.all([
    explorer.waitForEvent('close'),
    explorer.getByRole('button', { name: 'Use 3 selected records' }).click(),
  ]);
  for (const title of ['Sale', 'Autumn', 'Spring'])
    await expect(dialog.getByRole('checkbox', { name: title })).toBeChecked();
  await dialog.getByRole('button', { name: 'Apply' }).click();
  await expect
    .poll(async () => (await linked(record.id, 'categories')).length)
    .toBe(3);
  expect(await linked(record.id, 'categories')).toContain(sale.id);
});

test('a record created for a scoped relationship starts in its scope', async ({
  page,
}) => {
  const run = suffix();
  const group = await createRecordBlueprint(
    `picker_scope_group_${run}`,
    `Scope groups ${run}`,
    'attributes:\n  - code: title\n    value_type: string\n    tags: [searchable]',
  );
  const tools = await createRecord(group, [scalar('title', 'Tools')]);
  const option = await createRecordBlueprint(
    `picker_scope_option_${run}`,
    `Scope options ${run}`,
    `attributes:
  - code: title
    value_type: string
    tags: [searchable]
  - code: group
    value_type: relationship
    target_blueprint: picker_scope_group_${run}
    cardinality: one`,
  );
  const hammer = await createRecord(option, [
    scalar('title', 'Hammer'),
    relationship('group', tools.id),
  ]);
  const item = await createRecordBlueprint(
    `picker_scope_item_${run}`,
    `Scope items ${run}`,
    `attributes:
  - code: title
    value_type: string
  - code: group
    value_type: relationship
    target_blueprint: picker_scope_group_${run}
    cardinality: one
  - code: option
    value_type: relationship
    target_blueprint: picker_scope_option_${run}
    cardinality: one
    target_scope: {attribute: group, equals: group}`,
  );
  const record = await createRecord(item, [
    scalar('title', 'Kit'),
    relationship('group', tools.id),
  ]);
  await page.goto(`/records/${record.id}`);

  const { dialog, explorer } = await openExplorerPicker(page, 'option');
  await selectRow(explorer, 'Hammer');
  await explorer
    .getByRole('link', { name: `Create Scope options ${run}` })
    .click();
  // The scope's group is filled in already.
  await expect(explorer.getByText('Tools')).toBeVisible();
  await fieldByLabel(explorer, 'title').fill('Saw');
  await explorer.getByRole('button', { name: 'Create record' }).click();

  // A single-target picker replaces its earlier choice with the new record.
  await Promise.all([
    explorer.waitForEvent('close'),
    explorer.getByRole('button', { name: 'Use 1 selected record' }).click(),
  ]);
  await expect(dialog.getByRole('radio', { name: 'Saw' })).toBeChecked();
  await dialog.getByRole('button', { name: 'Apply' }).click();
  await expect
    .poll(async () => (await linked(record.id, 'option')).length)
    .toBe(1);
  expect(await linked(record.id, 'option')).not.toContain(hammer.id);
});
