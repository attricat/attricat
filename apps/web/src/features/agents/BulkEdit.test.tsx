// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApprovalCard } from './ApprovalCard';
import { BulkEditProgress } from './BulkEditProgress';
import { ConversationMessageContent } from './ConversationMessageContent';
import type { AgentToolCall } from './schemas';

vi.mock('@tanstack/react-router', () => ({
  Link: ({
    children,
    params,
    to,
  }: {
    children: ReactNode;
    params: { recordId: string };
    to: string;
  }) => <a href={to.replace('$recordId', params.recordId)}>{children}</a>,
}));
vi.mock('../records/api', () => ({
  getRecordLabels: (ids: string[]) =>
    Promise.resolve({
      items: ids.map((id) => ({
        id,
        blueprint_code: 'product',
        display: { default: `Product ${id.slice(-2)}` },
      })),
    }),
}));

const recordId = (index: number) =>
  `123e4567-e89b-12d3-a456-4266141740${String(index).padStart(2, '0')}`;

const bulkCall = (records: number): AgentToolCall => ({
  id: '123e4567-e89b-12d3-a456-426614174999',
  run_id: '123e4567-e89b-12d3-a456-426614174998',
  sequence: 1,
  provider_call_id: 'call_bulk',
  tool_name: 'bulk_set_record_values',
  arguments: {
    selection: { items: records },
    blueprint: { code: 'product', name: 'Product' },
    attributes: [
      { code: 'brand', name: 'Brand' },
      { code: 'season', name: 'Season' },
    ],
    records: Array.from({ length: records }, (_, index) => ({
      record_id: recordId(index),
      blueprint_code: 'product',
      label: `Boot ${index + 1}`,
      expected_updated_at: '2026-10-01T00:00:00Z',
      values: [
        {
          attribute_code: 'brand',
          context_id: null,
          value: 'Acme Co',
          previous: 'Acme',
        },
        {
          attribute_code: 'season',
          context_id: null,
          value: 'Spring',
          previous: null,
        },
      ],
    })),
  },
  change_summary: `Set Brand, Season on ${records} Product records.`,
  result: null,
  error: null,
  state: 'pending_approval',
  decided_by_user_id: null,
  decided_at: null,
  created_at: '2026-10-01T00:00:00Z',
  completed_at: null,
});

const renderWithQueries = (node: ReactNode) =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      {node}
    </QueryClientProvider>,
  );

describe('bulk edit approval', () => {
  it('previews the first records with old and new values, then all on request', async () => {
    const decide = vi.fn();
    renderWithQueries(
      <ApprovalCard call={bulkCall(25)} isDeciding={false} onDecide={decide} />,
    );
    expect(
      screen.getByText('Set Brand, Season on 25 Product records.'),
    ).toBeTruthy();
    expect(screen.getByText(/the edit stops there/)).toBeTruthy();
    const table = screen.getByRole('table', {
      name: 'Records this bulk edit changes',
    });
    expect(within(table).getAllByRole('link')).toHaveLength(20);
    const first = within(table).getByRole('link', { name: 'Boot 1' });
    expect(first.getAttribute('href')).toBe(`/records/${recordId(0)}`);
    const firstRows = within(table)
      .getAllByRole('row')
      .slice(1, 3)
      .map((row) => row.textContent);
    expect(firstRows[0]).toContain('Brand');
    expect(firstRows[0]).toContain('Acme');
    expect(firstRows[0]).toContain('Acme Co');
    expect(firstRows[1]).toContain('Season');
    expect(firstRows[1]).toContain('No value');
    expect(firstRows[1]).toContain('Spring');
    expect(within(table).queryByText('Boot 21')).toBeNull();

    await userEvent.click(
      screen.getByRole('button', { name: 'Show all 25 records' }),
    );
    expect(within(table).getAllByRole('link')).toHaveLength(25);
    expect(within(table).getByText('Boot 25')).toBeTruthy();
    await userEvent.click(
      screen.getByRole('button', { name: 'Show only the first 20 records' }),
    );
    expect(within(table).getAllByRole('link')).toHaveLength(20);

    await userEvent.click(screen.getByRole('button', { name: 'Approve' }));
    expect(decide).toHaveBeenCalledWith(bulkCall(25).id, true);
  });

  it('leaves other approvals without a bulk preview', () => {
    renderWithQueries(
      <ApprovalCard
        call={{
          ...bulkCall(1),
          tool_name: 'set_record_values',
          arguments: { record_id: recordId(0), values: [] },
        }}
        isDeciding={false}
        onDecide={vi.fn()}
      />,
    );
    expect(screen.queryByRole('table')).toBeNull();
  });

  it('shows how far a running edit has got', () => {
    render(
      <BulkEditProgress
        progress={{
          tool_call_id: recordId(90),
          run_id: recordId(91),
          record_id: recordId(3),
          state: 'applied',
          applied: 3,
          total: 12,
        }}
      />,
    );
    const bar = screen.getByRole('progressbar', {
      name: 'Applying the bulk edit: 3 of 12 records changed',
    });
    expect(bar.getAttribute('aria-valuenow')).toBe('25');
  });

  it('summarizes applied, failed and not run records with links', async () => {
    renderWithQueries(
      <ConversationMessageContent
        content={{
          tool_call_id: 'call_bulk',
          name: 'bulk_set_record_values',
          result: {
            applied: [recordId(1), recordId(2)],
            failed: [
              {
                record_id: recordId(3),
                code: 'stale_record',
                message: 'The record changed after it was read.',
              },
            ],
            not_run: [recordId(4)],
          },
        }}
        messageRole="tool"
      />,
    );
    expect(screen.getByText('2 records changed')).toBeTruthy();
    expect(screen.getByText('1 record failed')).toBeTruthy();
    expect(screen.getByText('1 record not changed')).toBeTruthy();
    expect(
      screen.getByText('stale_record: The record changed after it was read.'),
    ).toBeTruthy();
    const failed = await screen.findByRole('link', { name: 'Product 03' });
    expect(failed.getAttribute('href')).toBe(`/records/${recordId(3)}`);
    expect(screen.getByText('Show the 2 changed records')).toBeTruthy();
    expect(
      screen.getByText('Show the record that was not changed'),
    ).toBeTruthy();
  });
});
