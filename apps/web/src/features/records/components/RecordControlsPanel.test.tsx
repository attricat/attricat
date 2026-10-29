// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  render,
  renderHook,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import {
  getRecordApprovals,
  getRecordRetentionHolds,
  type RecordApproval,
  type RetentionHold,
} from '../api';
import { recordQueryKeys } from '../queryKeys';
import { RecordControlsPanel } from './RecordControlsPanel';
import { useHasRecordControls } from '../useHasRecordControls';

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
  approved_by_user_id: null,
  approved_by: 'Ada',
  approved_at: approvedAt,
  ended_at: null,
  end_reason: null,
  void_status: null,
  cause: null,
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

  it('explains the status change behind each approval', async () => {
    vi.mocked(getRecordApprovals).mockResolvedValue({
      items: [
        {
          ...approval(
            '00000000-0000-4000-8000-000000000002',
            '2026-10-20T10:00:00Z',
          ),
          cause: {
            from: 'review',
            transition_code: 'approve',
            kind: 'transition',
            operation: 'history_restore',
            channel: 'agent',
            separation_waived: true,
          },
        },
        {
          ...approval(
            '00000000-0000-4000-8000-000000000003',
            '2026-10-19T10:00:00Z',
          ),
          cause: {
            from: 'released',
            transition_code: null,
            kind: 'override',
            operation: 'edit',
            channel: 'person',
            separation_waived: false,
          },
        },
        approval(
          '00000000-0000-4000-8000-000000000004',
          '2026-10-18T10:00:00Z',
        ),
      ],
      has_more: false,
    });
    vi.mocked(getRecordRetentionHolds).mockResolvedValue({
      items: [],
      has_more: false,
    });
    renderPanel();
    const approvals = await screen.findByRole('region', { name: 'Approvals' });
    expect(
      within(approvals).getByText(
        'Entered from review through “approve” · restored from history · by an agent · separation of duties waived',
      ),
    ).toBeTruthy();
    expect(
      within(approvals).getByText(
        'Unlocked from released by an owner or administrator',
      ),
    ).toBeTruthy();
    expect(within(approvals).getByText('Cause not recorded')).toBeTruthy();
  });

  it('has controls only when the record has approvals or holds', async () => {
    const empty = { items: [], has_more: false };
    vi.mocked(getRecordApprovals).mockResolvedValue(empty);
    vi.mocked(getRecordRetentionHolds).mockResolvedValue(empty);
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const { result } = renderHook(() => useHasRecordControls(recordId), {
      wrapper: ({ children }) => (
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      ),
    });
    await waitFor(() =>
      expect(
        client.getQueryState(recordQueryKeys.retentionHolds(recordId))?.status,
      ).toBe('success'),
    );
    expect(result.current).toBe(false);

    vi.mocked(getRecordRetentionHolds).mockResolvedValue({
      items: [hold],
      has_more: false,
    });
    void client.invalidateQueries({
      queryKey: recordQueryKeys.recordControls(recordId),
    });
    await waitFor(() => expect(result.current).toBe(true));
  });
});
