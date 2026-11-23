import { expect, test, type Page } from '@playwright/test';
import {
  createRecord,
  createRecordBlueprint,
  recordSave,
  relationship,
  request,
  scalar,
  suffix,
} from './helpers';

type FormValue = {
  attribute_code: string;
  value?: unknown;
  target_record_id?: string;
};
type RecordForm = { values: FormValue[]; applied_movements: string[] };

const recordForm = (recordId: string) =>
  request<RecordForm>(`/v1/records/${recordId}`);

const valueOf = (form: RecordForm, code: string) =>
  form.values.find((value) => value.attribute_code === code)?.value;

const orderStatus = JSON.stringify({
  type: 'string',
  enum: ['draft', 'approved', 'completed', 'cancelled'],
  'x-attricat-status': {
    version: 1,
    options: [
      { code: 'draft', label: 'Draft' },
      { code: 'approved', label: 'Approved' },
      { code: 'completed', label: 'Completed' },
      { code: 'cancelled', label: 'Cancelled' },
    ],
    transitions: [
      { from: null, to: 'draft' },
      { from: 'draft', to: 'approved' },
      { from: 'approved', to: 'completed' },
      { from: 'approved', to: 'cancelled' },
    ],
  },
});

/**
 * Publishes materials, stock items with reservations, receipts and orders
 * that reserve on approval and consume on completion, and receives 10 of a
 * material.
 */
const production = async () => {
  const run = suffix();
  const codes = {
    material: `po_material_${run}`,
    stockItem: `po_stock_item_${run}`,
    receipt: `po_receipt_${run}`,
    order: `po_order_${run}`,
  };
  const material = await createRecordBlueprint(
    codes.material,
    `Materials ${run}`,
    'attributes:\n  - code: title\n    value_type: string\n    tags: [searchable]',
  );
  await createRecordBlueprint(
    codes.stockItem,
    `Stock items ${run}`,
    `unique_keys:
  - code: material
    attributes: [material]
attributes:
  - code: title
    value_type: string
  - code: material
    value_type: relationship
    target_blueprint: ${codes.material}
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    balance: {min: 0, reserved: reserved}
  - code: reserved
    value_type: number
    balance: {min: 0}`,
  );
  const target = (balance: string) =>
    `    target: {relationship: stock_item, blueprint: ${codes.stockItem}, balance: ${balance}, key: material, values: {material: material}}`;
  const receipt = await createRecordBlueprint(
    codes.receipt,
    `Receipts ${run}`,
    `attributes:
  - code: title
    value_type: string
  - code: material
    value_type: relationship
    target_blueprint: ${codes.material}
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: stock_item
    value_type: relationship
    target_blueprint: ${codes.stockItem}
    cardinality: one
movements:
  - code: stock
    direction: in
    quantity: quantity
    target: {relationship: stock_item, blueprint: ${codes.stockItem}, balance: quantity, key: material, values: {material: material}, create_missing: true}`,
  );
  const order = await createRecordBlueprint(
    codes.order,
    `Orders ${run}`,
    `attributes:
  - code: title
    value_type: string
  - code: status
    value_type: string
    value_schema: '${orderStatus}'
  - code: material
    value_type: relationship
    target_blueprint: ${codes.material}
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: stock_item
    value_type: relationship
    target_blueprint: ${codes.stockItem}
    cardinality: one
movements:
  - code: reserve
    direction: in
    quantity: quantity
${target('reserved')}
    apply_on: [{attribute: status, to: approved}]
  - code: release
    direction: out
    quantity: quantity
${target('reserved')}
    apply_on: [{attribute: status, to: completed}, {attribute: status, to: cancelled}]
    requires: [reserve]
  - code: consume
    direction: out
    quantity: quantity
${target('quantity')}
    apply_on: [{attribute: status, to: completed}]
    requires: [reserve]`,
  );
  const steel = await createRecord(material, [scalar('title', 'Steel')]);
  const received = await createRecord(receipt, [
    scalar('title', 'PZ/1'),
    relationship('material', steel.id),
    scalar('quantity', 10),
  ]);
  const stockItemId = (await recordForm(received.id)).values.find(
    (value) => value.attribute_code === 'stock_item',
  )!.target_record_id!;
  const newOrder = (title: string, quantity: number) =>
    createRecord(order, [
      scalar('title', title),
      scalar('status', 'draft'),
      relationship('material', steel.id),
      scalar('quantity', quantity),
    ]);
  return { newOrder, stockItemId };
};

/** The stock item's quantity and reserved balances. */
const stock = async (stockItemId: string) => {
  const form = await recordForm(stockItemId);
  return [valueOf(form, 'quantity'), valueOf(form, 'reserved') ?? 0];
};

/** Chooses a status option on the record page, which saves at once. */
const chooseStatus = async (page: Page, recordId: string, label: string) => {
  const saved = recordSave(page, recordId);
  await page.getByRole('combobox', { name: 'status', exact: true }).click();
  await page.getByRole('option', { name: label, exact: true }).click();
  return saved;
};

const quantityField = (page: Page) =>
  page.getByLabel('quantity', { exact: true });

for (const colorScheme of ['light', 'dark'] as const) {
  test(`approving an order reserves stock and completing consumes it in ${colorScheme} mode`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme });
    const { newOrder, stockItemId } = await production();
    const order = await newOrder('ZP/1', 6);

    // A draft order holds nothing, and its quantity can still change.
    await page.goto(`/records/${order.id}`);
    await expect(quantityField(page)).toBeVisible();
    await expect(page.getByText(/already (used|posted)/)).toHaveCount(0);

    expect((await chooseStatus(page, order.id, 'Approved')).ok()).toBe(true);
    await expect.poll(() => stock(stockItemId)).toEqual([10, 6]);
    // The approval reserved the quantity, so it no longer changes.
    await expect(
      page.getByText(/^This record's movement already used .*quantity/),
    ).toBeVisible();
    await expect(quantityField(page)).toHaveCount(0);

    // Another order cannot reserve what is already reserved.
    const second = await newOrder('ZP/2', 5);
    await page.goto(`/records/${second.id}`);
    expect((await chooseStatus(page, second.id, 'Approved')).status()).toBe(
      409,
    );
    await expect(
      page.getByText(
        "Not enough available: 4 free, 5 requested. The balance can't go below 0.",
      ),
    ).toBeVisible();
    expect((await recordForm(second.id)).applied_movements).toEqual([]);
    expect(await stock(stockItemId)).toEqual([10, 6]);

    // The stock item shows both balances as values.
    await page.goto(`/records/${stockItemId}`);
    await expect(page.getByText('6', { exact: true })).toBeVisible();
    await expect(page.getByText('10', { exact: true })).toBeVisible();

    // Completion releases the reservation and consumes the material.
    await page.goto(`/records/${order.id}`);
    expect((await chooseStatus(page, order.id, 'Completed')).ok()).toBe(true);
    await expect.poll(() => stock(stockItemId)).toEqual([4, 0]);
    expect((await recordForm(order.id)).applied_movements).toEqual([
      'consume',
      'release',
      'reserve',
    ]);
  });
}

/**
 * Publishes a production order whose material lines reserve stock on
 * approval, listed on the order's page as an editable table, and receives
 * 10 of a material.
 */
const productionWithLines = async () => {
  const run = suffix();
  const codes = {
    material: `pl_material_${run}`,
    stockItem: `pl_stock_item_${run}`,
    receipt: `pl_receipt_${run}`,
    production: `pl_production_${run}`,
    line: `pl_line_${run}`,
  };
  const material = await createRecordBlueprint(
    codes.material,
    `Line materials ${run}`,
    'attributes:\n  - code: title\n    value_type: string\n    tags: [searchable]',
  );
  await createRecordBlueprint(
    codes.stockItem,
    `Line stock items ${run}`,
    `unique_keys:
  - code: material
    attributes: [material]
attributes:
  - code: title
    value_type: string
  - code: material
    value_type: relationship
    target_blueprint: ${codes.material}
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    balance: {min: 0, reserved: reserved}
  - code: reserved
    value_type: number
    balance: {min: 0}`,
  );
  const receipt = await createRecordBlueprint(
    codes.receipt,
    `Line receipts ${run}`,
    `attributes:
  - code: title
    value_type: string
  - code: material
    value_type: relationship
    target_blueprint: ${codes.material}
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: stock_item
    value_type: relationship
    target_blueprint: ${codes.stockItem}
    cardinality: one
movements:
  - code: stock
    direction: in
    quantity: quantity
    target: {relationship: stock_item, blueprint: ${codes.stockItem}, balance: quantity, key: material, values: {material: material}, create_missing: true}`,
  );
  const production = await createRecordBlueprint(
    codes.production,
    `Productions ${run}`,
    `views:
  detail:
    type: stack
    children:
      - {type: field, field: title}
      - {type: field, field: status}
      - type: incoming_relationship_list
        label: Material lines
        display: table
        editable: true
        columns: [material, quantity]
        relationships:
          - {source_blueprint: ${codes.line}, field: production}
        page_size: 50
attributes:
  - code: title
    value_type: string
  - code: status
    value_type: string
    value_schema: '${orderStatus}'`,
  );
  await createRecordBlueprint(
    codes.line,
    `Material lines ${run}`,
    `attributes:
  - code: title
    value_type: string
  - code: production
    value_type: relationship
    target_blueprint: ${codes.production}
    cardinality: one
    context_editable: default
  - code: material
    value_type: relationship
    target_blueprint: ${codes.material}
    cardinality: one
    context_editable: default
  - code: quantity
    value_type: number
    context_editable: default
  - code: stock_item
    value_type: relationship
    target_blueprint: ${codes.stockItem}
    cardinality: one
movements:
  - code: reserve
    direction: in
    quantity: quantity
    target: {relationship: stock_item, blueprint: ${codes.stockItem}, balance: reserved, key: material, values: {material: material}}
    apply_on: [{relationship: production, attribute: status, to: approved}]`,
  );
  const steel = await createRecord(material, [scalar('title', 'Steel')]);
  const received = await createRecord(receipt, [
    scalar('title', 'PZ/1'),
    relationship('material', steel.id),
    scalar('quantity', 10),
  ]);
  const stockItemId = (await recordForm(received.id)).values.find(
    (value) => value.attribute_code === 'stock_item',
  )!.target_record_id!;
  const order = await createRecord(production, [
    scalar('title', 'ZP/1'),
    scalar('status', 'draft'),
  ]);
  return { order, run, stockItemId };
};

for (const colorScheme of ['light', 'dark'] as const) {
  test(`adds and edits an order's lines on its page until approval in ${colorScheme} mode`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme });
    const { order, run, stockItemId } = await productionWithLines();
    await page.goto(`/records/${order.id}`);
    const lines = page.getByRole('table', { name: 'Material lines' });
    await expect(lines.getByText('Nothing here yet.')).toBeVisible();

    // Two lines added in place; the order is set and cannot be changed.
    const addLine = async (quantity: string, another: boolean) => {
      const dialog = page.getByRole('dialog', {
        name: `Add Material lines ${run}`,
      });
      await page
        .getByRole('button', { name: `Add Material lines ${run}` })
        .click();
      await dialog.getByRole('button', { name: 'Choose material' }).click();
      const picker = page.getByRole('dialog', { name: /^Select / });
      await picker.getByRole('radio', { name: 'Steel' }).check();
      await picker.getByRole('button', { name: 'Apply' }).click();
      await dialog.getByLabel('quantity', { exact: true }).fill(quantity);
      await dialog
        .getByRole('button', {
          name: another ? 'Add and add another' : 'Add',
          exact: true,
        })
        .click();
      if (!another) await expect(dialog).toBeHidden();
    };
    await addLine('4', false);
    await addLine('9', false);
    await expect(lines.getByRole('row')).toHaveCount(3);
    const quantities = lines.getByRole('spinbutton', { name: 'quantity' });
    await expect(quantities).toHaveCount(2);

    // A cell edits its line in place and saves when it is left.
    await quantities.first().fill('5');
    await quantities.first().press('Enter');
    await expect(lines.getByText('Saved')).toBeVisible();

    // A draft's line can still be removed.
    await lines
      .getByRole('button', { name: /^Delete / })
      .last()
      .click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Delete record' })
      .click();
    await expect(lines.getByRole('row')).toHaveCount(2);

    // Approval reserves the edited line and settles the table.
    expect((await chooseStatus(page, order.id, 'Approved')).ok()).toBe(true);
    await expect.poll(() => stock(stockItemId)).toEqual([10, 5]);
    await expect(
      page.getByText(/were posted with the record's status/),
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: `Add Material lines ${run}` }),
    ).toHaveCount(0);
    await expect(lines.getByRole('button', { name: /^Delete / })).toHaveCount(
      0,
    );
    await expect(lines.getByRole('spinbutton')).toHaveCount(0);
    await expect(lines.getByText('5', { exact: true })).toBeVisible();

    // The line still opens beside the order, with its applied values locked.
    await lines.getByRole('button', { name: /^Open / }).click();
    const panel = page.getByRole('complementary');
    await expect(
      panel.getByText(/^This record's movement already used .*quantity/),
    ).toBeVisible();
  });
}
