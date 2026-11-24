import { expect, test, type Page } from '@playwright/test';
import {
  createRecord,
  createRecordBlueprint,
  fieldByLabel,
  formValue,
  quantityField,
  recordForm,
  relationship,
  scalar,
  suffix,
} from './helpers';

/** The stock level of the stock item a receipt or issue changed. */
const stockLevel = async (movementRecordId: string) => {
  const stockItem = formValue(await recordForm(movementRecordId), 'stock_item');
  if (!stockItem?.target_record_id) throw new Error('no stock item linked');
  return formValue(await recordForm(stockItem.target_record_id), 'quantity')
    ?.value;
};

/**
 * Publishes products, unit conversions, stock items, goods receipts and
 * goods issues under codes unique to this run.
 */
const inventory = async () => {
  const run = suffix();
  const codes = {
    product: `sm_product_${run}`,
    conversion: `sm_conversion_${run}`,
    stockItem: `sm_stock_item_${run}`,
    receipt: `sm_receipt_${run}`,
    issue: `sm_issue_${run}`,
  };
  const product = await createRecordBlueprint(
    codes.product,
    `Products ${run}`,
    'attributes:\n  - code: title\n    value_type: string\n    tags: [searchable]',
  );
  const conversion = await createRecordBlueprint(
    codes.conversion,
    `Unit conversions ${run}`,
    `attributes:
  - code: title
    value_type: string
    tags: [searchable]
  - code: product
    value_type: relationship
    target_blueprint: ${codes.product}
    cardinality: one
  - code: factor
    value_type: number`,
  );
  await createRecordBlueprint(
    codes.stockItem,
    `Stock items ${run}`,
    `unique_keys:
  - code: product
    attributes: [product]
attributes:
  - code: title
    value_type: string
  - code: product
    value_type: relationship
    target_blueprint: ${codes.product}
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    balance: {min: 0}`,
  );
  const movementBlueprint = (
    code: string,
    name: string,
    direction: 'in' | 'out',
  ) =>
    createRecordBlueprint(
      code,
      name,
      `attributes:
  - code: title
    value_type: string
  - code: product
    value_type: relationship
    target_blueprint: ${codes.product}
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: conversion
    value_type: relationship
    target_blueprint: ${codes.conversion}
    cardinality: one
    context_editable: default
    target_scope: {attribute: product, equals: product}
  - code: stock_item
    value_type: relationship
    target_blueprint: ${codes.stockItem}
    cardinality: one
  - code: base_quantity
    value_type: number
  - code: reverses
    value_type: relationship
    target_blueprint: ${code}
    cardinality: one
    context_editable: default
movements:
  - code: stock
    direction: ${direction}
    quantity: quantity
    factor: {relationship: conversion, attribute: factor}
    result: base_quantity
    target:
      relationship: stock_item
      blueprint: ${codes.stockItem}
      balance: quantity
      key: product
      values: {product: product}
      create_missing: ${direction === 'in'}
    reversal: {relationship: reverses}`,
    );
  await movementBlueprint(codes.receipt, `Goods receipts ${run}`, 'in');
  await movementBlueprint(codes.issue, `Goods issues ${run}`, 'out');
  const bolt = await createRecord(product, [scalar('title', 'Bolt')]);
  const nut = await createRecord(product, [scalar('title', 'Nut')]);
  await createRecord(conversion, [
    scalar('title', 'Box of 12'),
    relationship('product', bolt.id),
    scalar('factor', 12),
  ]);
  await createRecord(conversion, [
    scalar('title', 'Crate of nuts'),
    relationship('product', nut.id),
    scalar('factor', 100),
  ]);
  return { codes, run };
};

/** The quantity input; `base quantity` also contains the word. */
/** Opens the create form of a blueprint. */
const newRecord = async (page: Page, name: string, code: string) => {
  await page.goto('/records/new');
  await page.getByLabel('Blueprint').click();
  await page.getByRole('option', { name: `${name} (${code})` }).click();
  await page.getByRole('button', { name: 'Load blueprint' }).click();
};

/** Picks one record in a single-target relationship field. */
const pick = async (page: Page, field: string, option: string) => {
  await page.getByRole('button', { name: `Choose ${field}` }).click();
  const dialog = page.getByRole('dialog', { name: /^Select / });
  await dialog.getByRole('radio', { name: option }).check();
  await dialog.getByRole('button', { name: 'Apply' }).click();
};

for (const colorScheme of ['light', 'dark'] as const) {
  test(`receives, issues and reverses stock in ${colorScheme} mode`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme });
    const { codes, run } = await inventory();

    // A receipt of 2 boxes of 12: the conversion picker offers only the
    // conversions of the selected product.
    await newRecord(page, `Goods receipts ${run}`, codes.receipt);
    await fieldByLabel(page, 'title').fill('PZ/1');
    await pick(page, 'product', 'Bolt');
    await quantityField(page).fill('2');
    await page.getByRole('button', { name: 'Choose conversion' }).click();
    const conversions = page.getByRole('dialog', { name: /^Select / });
    await expect(
      conversions.getByRole('radio', { name: 'Box of 12' }),
    ).toBeVisible();
    await expect(
      conversions.getByRole('radio', { name: 'Crate of nuts' }),
    ).toHaveCount(0);
    await conversions.getByRole('radio', { name: 'Box of 12' }).check();
    await conversions.getByRole('button', { name: 'Apply' }).click();
    await page.getByRole('button', { name: 'Create record' }).click();
    await expect(page).toHaveURL(/\/records\/[0-9a-f-]{36}$/);
    const receiptId = page.url().split('/').at(-1)!;
    expect(await stockLevel(receiptId)).toBe(24);

    // The saved receipt keeps the quantity its movement applied.
    await expect(
      page.getByText(/^This record's movement already used .*quantity/),
    ).toBeVisible();
    await expect(quantityField(page)).toHaveCount(0);

    // Issuing more than is in stock is refused, and nothing is saved.
    await newRecord(page, `Goods issues ${run}`, codes.issue);
    await fieldByLabel(page, 'title').fill('WZ/1');
    await pick(page, 'product', 'Bolt');
    await quantityField(page).fill('30');
    await page.getByRole('button', { name: 'Create record' }).click();
    await expect(
      page.getByText(
        "Not enough available: 24 free, 30 requested. The balance can't go below 0.",
      ),
    ).toBeVisible();
    await expect(page).toHaveURL(/\/records\/new/);

    // A smaller issue goes through and lowers the stock level.
    await quantityField(page).fill('5');
    await page.getByRole('button', { name: 'Create record' }).click();
    await expect(page).toHaveURL(/\/records\/[0-9a-f-]{36}$/);
    const issueId = page.url().split('/').at(-1)!;
    expect(await stockLevel(issueId)).toBe(19);

    // The stock item shows the new level.
    const stockItemId = formValue(
      await recordForm(issueId),
      'stock_item',
    )!.target_record_id!;
    await page.goto(`/records/${stockItemId}`);
    // Only movements change it, so the page shows it as a value.
    await expect(page.getByText('19', { exact: true })).toBeVisible();
    await expect(quantityField(page)).toHaveCount(0);

    // Reversing the issue puts the 5 back and opens the reversal.
    await page.goto(`/records/${issueId}`);
    await page.getByRole('button', { name: 'Actions' }).click();
    await page.getByRole('menuitem', { name: 'Reverse record' }).click();
    await page
      .getByRole('dialog', { name: 'Reverse record' })
      .getByRole('button', { name: 'Reverse record' })
      .click();
    await expect(page).not.toHaveURL(new RegExp(issueId));
    const reversalId = page.url().split('/').at(-1)!;
    expect(await stockLevel(reversalId)).toBe(24);
    // A reversal cannot itself be reversed.
    await page.getByRole('button', { name: 'Actions' }).click();
    await expect(
      page.getByRole('menuitem', { name: 'Reverse record' }),
    ).toHaveCount(0);
    await page.keyboard.press('Escape');

    // The original can be reversed only once.
    await page.goto(`/records/${issueId}`);
    await page.getByRole('button', { name: 'Actions' }).click();
    await page.getByRole('menuitem', { name: 'Reverse record' }).click();
    const again = page.getByRole('dialog', { name: 'Reverse record' });
    await again.getByRole('button', { name: 'Reverse record' }).click();
    await expect(
      again.getByText('This record has already been reversed.'),
    ).toBeVisible();
    expect(await stockLevel(issueId)).toBe(24);
  });
}
