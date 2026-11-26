// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { listContexts } from '../../contexts/api';
import {
  createRecord,
  getBlueprintByCode,
  getDefaultValues,
  getRecordLabels,
  getRecordForm,
  getResolvedRecordPreview,
  searchRecords,
  updateRecord,
} from '../../records/api';
import { RecordWritesContext } from '../recordWrites';
import { IncomingRelationshipTable } from './IncomingRelationshipTable';
import { incomingTableColumns } from './incomingTableColumns';

vi.mock('../../contexts/api', async (importOriginal) => ({
  ...(await importOriginal()),
  listContexts: vi.fn(),
}));
vi.mock('../../records/api', async (importOriginal) => ({
  ...(await importOriginal()),
  createRecord: vi.fn(),
  getBlueprintByCode: vi.fn(),
  getDefaultValues: vi.fn(),
  getRecordLabels: vi.fn(),
  getRecordForm: vi.fn(),
  getResolvedRecordPreview: vi.fn(),
  searchRecords: vi.fn(),
  updateRecord: vi.fn(),
}));

const orderId = '0190a6f2-7c1e-7b3a-9c4d-2e5f6a7b8c9d';
const lineId = '0190a6f2-7c1e-7b3a-9c4d-2e5f6a7b8c9e';
const defaultContextId = '00000000-0000-4000-8000-000000000001';

const lineBlueprint = {
  blueprint: {
    id: '0190a6f2-7c1e-7b3a-9c4d-2e5f6a7b8c90',
    code: 'order_line',
    name: 'Order line',
    version: 1,
    status: 'published',
    views: {},
  },
  attributes: [
    { code: 'order', name: 'Order', value_type: 'relationship' },
    { code: 'quantity', name: 'Quantity', value_type: 'number' },
    { code: 'note', name: 'Note', value_type: 'string' },
  ],
  table_path_attributes: [],
};

const node = {
  type: 'incoming_relationship_list' as const,
  label: 'Lines',
  page_size: 50,
  relationships: [{ source_blueprint: 'order_line', field: 'order' }],
  display: 'table' as const,
  editable: true,
};

const lineForm = {
  can_write: true,
  record: {
    id: lineId,
    blueprint_id: lineBlueprint.blueprint.id,
    blueprint_version: 1,
    updated_at: '2026-11-23T10:00:00Z',
  },
  blueprint: lineBlueprint,
  values: [
    {
      kind: 'scalar',
      attribute_code: 'quantity',
      context_id: defaultContextId,
      value: 6,
    },
  ],
  reusable_attributes: [],
  reusable_values: [],
  applied_movements: [],
  settled_lines: [],
  context: {},
};

const renderTable = (settled: boolean, canWrite = true, writes = true) => {
  vi.mocked(getRecordForm).mockImplementation(
    async (id: string) =>
      (id === lineId
        ? lineForm
        : {
            blueprint: lineBlueprint,
            can_write: canWrite,
            settled_lines: settled
              ? [{ blueprint: 'order_line', relationship: 'order' }]
              : [],
          }) as never,
  );
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <RecordWritesContext value={writes}>
        <IncomingRelationshipTable node={node} recordId={orderId} />
      </RecordWritesContext>
    </QueryClientProvider>,
  );
};

beforeEach(() => {
  vi.mocked(listContexts).mockResolvedValue([
    { id: defaultContextId, code: 'default' },
  ] as never);
  vi.mocked(searchRecords).mockResolvedValue({
    blueprint: lineBlueprint,
    items: [{ id: lineId, display: { default: 'Line 1' } }],
    next_cursor: null,
    total_count: 1,
  } as never);
  vi.mocked(getResolvedRecordPreview).mockResolvedValue({
    values: {
      quantity: {
        value: 6,
        source_context: { id: defaultContextId, code: 'default' },
      },
    },
  } as never);
});

describe('incomingTableColumns', () => {
  it("leaves out the link back and follows the node's columns", () => {
    const codes = (columns?: string[]) =>
      incomingTableColumns(lineBlueprint as never, { columns }, 'order').map(
        (attribute) => attribute.code,
      );
    expect(codes()).toEqual(['quantity', 'note']);
    expect(codes(['note', 'order', 'missing'])).toEqual(['note']);
  });
});

describe('IncomingRelationshipTable', () => {
  it('lists the records linking here with their values', async () => {
    renderTable(false);

    const table = await screen.findByRole('table', { name: 'Lines' });
    expect(
      within(table)
        .getAllByRole('columnheader')
        .map((header) => header.textContent),
    ).toEqual(['Quantity', 'Note', 'Actions']);
    expect(
      await within(table).findByRole('spinbutton', { name: 'Quantity' }),
    ).toHaveProperty('value', '6');
    expect(searchRecords).toHaveBeenCalledWith(
      expect.objectContaining({
        blueprint: 'order_line',
        pageSize: 50,
        relationshipFilters: [
          { field: 'order', selected_target_ids: [orderId] },
        ],
      }),
    );
    expect(
      await screen.findByRole('button', { name: 'Add Order line' }),
    ).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Delete Line 1' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Open Line 1' })).toBeTruthy();
  });

  it('saves a changed cell on its own', async () => {
    vi.mocked(updateRecord).mockResolvedValue({
      updated_at: '2026-11-23T10:01:00Z',
    } as never);
    renderTable(false);

    const quantity = await screen.findByRole('spinbutton', {
      name: 'Quantity',
    });
    fireEvent.focus(quantity);
    fireEvent.change(quantity, { target: { value: '8' } });
    fireEvent.blur(quantity);

    await waitFor(() => expect(updateRecord).toHaveBeenCalled());
    expect(vi.mocked(updateRecord).mock.calls[0][0]).toBe(lineId);
    expect(JSON.stringify(vi.mocked(updateRecord).mock.calls[0])).toContain(
      '"quantity"',
    );
  });

  it('adds a record as a new row with the link set', async () => {
    vi.mocked(getBlueprintByCode).mockResolvedValue(lineBlueprint as never);
    vi.mocked(createRecord).mockResolvedValue({ id: lineId } as never);
    renderTable(false);

    (await screen.findByRole('button', { name: 'Add Order line' })).click();
    const row = await screen.findByRole('row', { name: 'New Order line' });
    const quantity = await within(row).findByRole('spinbutton', {
      name: 'Quantity',
    });
    await waitFor(() => expect(document.activeElement).toBe(quantity));
    fireEvent.change(quantity, { target: { value: '3' } });
    fireEvent.keyDown(quantity, { key: 'Enter' });

    await waitFor(() => expect(createRecord).toHaveBeenCalled());
    const { values } = vi.mocked(createRecord).mock.calls[0][0];
    expect(values).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ attribute_code: 'quantity', value: 3 }),
        expect.objectContaining({
          attribute_code: 'order',
          target_record_id: orderId,
        }),
      ]),
    );
    // The row empties for the next record; Escape closes it.
    await waitFor(() => expect(quantity).toHaveProperty('value', ''));
    fireEvent.keyDown(quantity, { key: 'Escape' });
    expect(screen.queryByRole('row', { name: 'New Order line' })).toBeNull();
  });

  it('fills a new row with the values its link defaults, and saves them', async () => {
    // A required currency the line copies from its order.
    const defaulted = {
      ...lineBlueprint,
      blueprint: {
        ...lineBlueprint.blueprint,
        record_schema: { type: 'object', required: ['currency'] },
      },
      attributes: [
        ...lineBlueprint.attributes,
        {
          code: 'currency',
          name: 'Currency',
          value_type: 'string',
          default_from: 'order.currency',
        },
      ],
    };
    vi.mocked(getBlueprintByCode).mockResolvedValue(defaulted as never);
    vi.mocked(searchRecords).mockResolvedValue({
      blueprint: defaulted,
      items: [],
      next_cursor: null,
      total_count: 0,
    } as never);
    vi.mocked(getDefaultValues).mockResolvedValue({
      values: [{ attribute_code: 'currency', value: 'PLN' }],
    });
    vi.mocked(getRecordLabels).mockResolvedValue({
      items: [
        {
          id: orderId,
          blueprint_code: 'order',
          display: { default: 'Order 7' },
        },
      ],
    });
    vi.mocked(createRecord).mockResolvedValue({ id: lineId } as never);
    renderTable(false);

    (await screen.findByRole('button', { name: 'Add Order line' })).click();
    const row = await screen.findByRole('row', { name: 'New Order line' });
    const currency = await within(row).findByRole('textbox', {
      name: 'Currency',
    });
    await waitFor(() => expect(currency).toHaveProperty('value', 'PLN'));
    expect(getDefaultValues).toHaveBeenCalledWith(
      expect.objectContaining({
        relationship: 'order',
        targetRecordId: orderId,
      }),
      expect.any(AbortSignal),
    );
    expect(await within(row).findByText(/Suggested from Order 7/)).toBeTruthy();

    // The required, defaulted field no longer blocks the save.
    fireEvent.keyDown(currency, { key: 'Enter' });
    await waitFor(() => expect(createRecord).toHaveBeenCalled());
    expect(vi.mocked(createRecord).mock.calls[0][0].values).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ attribute_code: 'currency', value: 'PLN' }),
      ]),
    );
    // The next row starts with the defaults again.
    await waitFor(() => expect(getDefaultValues).toHaveBeenCalledTimes(2));
  });

  it('only opens settled records', async () => {
    renderTable(true);

    expect(
      await screen.findByText(/were posted with the record's status/),
    ).toBeTruthy();
    expect(
      await screen.findByRole('button', { name: 'Open Line 1' }),
    ).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Add Order line' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Delete Line 1' })).toBeNull();
    expect(await screen.findByText('6')).toBeTruthy();
    expect(screen.queryByRole('spinbutton')).toBeNull();
  });

  it('offers no changes without write access', async () => {
    renderTable(false, false);

    expect(
      await screen.findByRole('button', { name: 'Open Line 1' }),
    ).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Add Order line' })).toBeNull();
  });

  it('offers no changes where views may not change records', async () => {
    renderTable(false, true, false);

    expect(
      await screen.findByRole('button', { name: 'Open Line 1' }),
    ).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Add Order line' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Delete Line 1' })).toBeNull();
  });
});
