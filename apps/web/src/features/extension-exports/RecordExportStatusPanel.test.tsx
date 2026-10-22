// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { RecordExportStatusPanel } from './RecordExportStatusPanel';

const recordId = '00000000-0000-4000-8000-000000000001';

vi.mock('../auth/useCan', () => ({ useCan: () => true }));
vi.mock('./api', () => ({
  recordExportStatus: vi.fn(),
  retryExports: vi.fn(async () => ({ record_ids: [recordId], job_errors: [] })),
  previewExport: vi.fn(async () => ({
    payload: { title: 'Chair' },
    issues: ['GTIN is required'],
  })),
}));

const destination = (overrides = {}) => ({
  extension_id: 'acme.shop',
  extension_name: 'Shop exporter',
  export_id: 'shop',
  title: 'Shop',
  connection_kind: 'shop',
  preview: true,
  resync_operation: null,
  reconcile_operation: null,
  ...overrides,
});

const mapping = {
  extension_id: 'acme.shop',
  connection: 'shop-eu',
  record_id: recordId,
  external_id: 'gid://shop/Product/1',
  external_ids: {},
  content_hash: null,
  record_revision: 7,
  status: 'rejected',
  error: { code: 'missing_gtin', message: 'GTIN is required' },
  attempts: 1,
  retry_requested_at: null,
  synced_at: null,
  updated_at: '2026-10-21T10:00:00Z',
} as const;

const renderPanel = () =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <RecordExportStatusPanel recordId={recordId} />
    </QueryClientProvider>,
  );

describe('RecordExportStatusPanel', () => {
  beforeEach(() => vi.clearAllMocks());

  it('lists the status, error and actions at each destination', async () => {
    const api = await import('./api');
    vi.mocked(api.recordExportStatus).mockResolvedValue({
      statuses: [mapping],
      destinations: [
        destination({
          export_id: 'other',
          title: 'Other',
          connection_kind: 'other',
        }),
        destination(),
        destination({
          extension_id: 'acme.feed',
          title: 'Feed',
          preview: false,
        }),
      ],
      connections: [
        {
          extension_id: 'acme.shop',
          connection: 'shop-eu',
          kind: 'shop',
          display_name: 'Shop EU',
        },
      ],
    });
    const user = userEvent.setup();
    renderPanel();

    const rows = await screen.findAllByRole('row');
    expect(rows).toHaveLength(4);
    // The mapping's connection kind picks its destination.
    expect(within(rows[1]).getByText('Shop')).toBeTruthy();
    expect(within(rows[1]).getByText('Rejected')).toBeTruthy();
    expect(
      within(rows[1]).getByText('missing_gtin: GTIN is required'),
    ).toBeTruthy();
    expect(within(rows[1]).getByText('Shop EU')).toBeTruthy();
    // Destinations without a mapping still list the record as not exported.
    expect(within(rows[2]).getByText('Other')).toBeTruthy();
    expect(within(rows[3]).getByText('Not exported yet')).toBeTruthy();
    expect(
      within(rows[3]).queryByRole('button', { name: 'Preview' }),
    ).toBeNull();

    await user.click(within(rows[1]).getByRole('button', { name: 'Retry' }));
    expect(api.retryExports).toHaveBeenCalledWith('acme.shop', {
      connection: 'shop-eu',
      recordIds: [recordId],
    });

    await user.click(within(rows[1]).getByRole('button', { name: 'Preview' }));
    expect(await screen.findByText('GTIN is required')).toBeTruthy();
    expect(api.previewExport).toHaveBeenCalledWith(
      'acme.shop',
      'shop',
      recordId,
      'shop-eu',
    );
  });

  it('explains when no export extension is enabled', async () => {
    const api = await import('./api');
    vi.mocked(api.recordExportStatus).mockResolvedValue({
      statuses: [],
      destinations: [],
      connections: [],
    });
    renderPanel();
    expect(await screen.findByText('No export destinations')).toBeTruthy();
  });
});
