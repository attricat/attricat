// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { listContexts } from '../../contexts/api';
import {
  getRecordForm,
  getResolvedRecordPreview,
  searchRecords,
} from '../../records/api';
import { IncomingRelationshipTable } from './IncomingRelationshipTable';
import { incomingTableColumns } from './incomingTableColumns';

vi.mock('../../contexts/api', async (importOriginal) => ({
  ...(await importOriginal()),
  listContexts: vi.fn(),
}));
vi.mock('../../records/api', async (importOriginal) => ({
  ...(await importOriginal()),
  getRecordForm: vi.fn(),
  getResolvedRecordPreview: vi.fn(),
  searchRecords: vi.fn(),
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

const renderTable = (settled: boolean, canWrite = true) => {
  vi.mocked(getRecordForm).mockResolvedValue({
    blueprint: lineBlueprint,
    can_write: canWrite,
    settled_lines: settled
      ? [{ blueprint: 'order_line', relationship: 'order' }]
      : [],
  } as never);
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <IncomingRelationshipTable node={node} recordId={orderId} />
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
    expect(await within(table).findByText('6')).toBeTruthy();
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
  });

  it('offers no changes without write access', async () => {
    renderTable(false, false);

    expect(
      await screen.findByRole('button', { name: 'Open Line 1' }),
    ).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Add Order line' })).toBeNull();
  });
});
