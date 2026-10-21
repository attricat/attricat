// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import {
  getRecordApprovals,
  getRecordRetentionHolds,
  type RecordApproval,
  type RetentionHold,
} from '../api';
import { RecordControlsPanel } from './RecordControlsPanel';

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  getRecordApprovals: vi.fn(),
  getRecordRetentionHolds: vi.fn(),
}));

const recordId = '00000000-0000-4000-8000-000000000001';
const approval = (id: string, approvedAt: string): RecordApproval => ({
  id,
  attribute_code: 'status',
  context_code: 'default',
  status: 'approved',
  covers_all: true,
  covered_attributes: [],
  content_digest: 'a'.repeat(64),
  approved_by: 'Ada',
  approved_at: approvedAt,
  ended_at: null,
  end_reason: null,
  void_status: null,
});
const hold: RetentionHold = {
  id: '00000000-0000-4000-8000-000000000010',
  file_id: '00000000-0000-4000-8000-000000000011',
  source: 'explicit',
  attribute_code: null,
  status: null,
  reason: 'Legal hold',
  held_until: '2027-10-20T10:00:00Z',
  created_at: '2026-10-20T10:00:00Z',
  released_at: null,
  active: true,
};

const renderPanel = () =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <RecordControlsPanel attributes={[]} recordId={recordId} />
    </QueryClientProvider>,
  );

describe('RecordControlsPanel', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('loads older approvals before the last one shown', async () => {
    const user = userEvent.setup();
    const newest = approval(
      '00000000-0000-4000-8000-000000000002',
      '2026-10-20T10:00:00Z',
    );
    const older = approval(
      '00000000-0000-4000-8000-000000000003',
      '2026-10-19T10:00:00Z',
    );
    vi.mocked(getRecordApprovals).mockImplementation((_, cursor) =>
      Promise.resolve(
        cursor
          ? { items: [older], has_more: false }
          : { items: [newest], has_more: true },
      ),
    );
    vi.mocked(getRecordRetentionHolds).mockResolvedValue({
      items: [hold],
      has_more: false,
    });
    renderPanel();
    const approvals = await screen.findByRole('region', { name: 'Approvals' });
    expect(within(approvals).getAllByRole('listitem')).toHaveLength(1);
    expect(
      within(screen.getByRole('region', { name: 'Retention holds' })).getByText(
        'Retained: Legal hold',
      ),
    ).toBeTruthy();
    expect(
      screen.queryByRole('button', { name: 'Load more retention holds' }),
    ).toBeNull();

    await user.click(
      within(approvals).getByRole('button', { name: 'Load more approvals' }),
    );
    expect(await within(approvals).findAllByRole('listitem')).toHaveLength(2);
    expect(getRecordApprovals).toHaveBeenLastCalledWith(
      recordId,
      { before_time: newest.approved_at, before_id: newest.id },
      expect.anything(),
    );
    expect(
      within(approvals).queryByRole('button', { name: 'Load more approvals' }),
    ).toBeNull();
  });

  it('says when the record has no approvals or holds', async () => {
    const empty = { items: [], has_more: false };
    vi.mocked(getRecordApprovals).mockResolvedValue(empty);
    vi.mocked(getRecordRetentionHolds).mockResolvedValue(empty);
    renderPanel();
    expect(
      await screen.findByText('No approvals or retention holds.'),
    ).toBeTruthy();
  });
});
