// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { authQueryKeys } from '../../auth/queryKeys';
import {
  unlockRecordStatus,
  type Attribute,
  type StatusTransitionAccess,
} from '../api';
import { savedStatusLocks } from '../status';
import { StatusLockNotice } from './StatusLockNotice';

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  unlockRecordStatus: vi.fn(),
}));

const recordId = '123e4567-e89b-12d3-a456-426614174010';
const updatedAt = '2026-10-21T09:00:00Z';

const saleStatus = (lock: 'all' | string[]): Attribute => ({
  code: 'sale_status',
  name: 'Sale status',
  value_type: 'string',
  value_schema: {
    type: 'string',
    enum: ['reserved', 'sold', 'archived'],
    'x-attricat-status': {
      version: 1,
      options: [
        { code: 'reserved', label: 'Reserved' },
        { code: 'sold', label: 'Sold', lock },
        { code: 'archived', label: 'Archived', lock: 'all' },
      ],
      transitions: [
        { from: 'reserved', to: 'sold', code: 'sell' },
        { from: 'sold', to: 'reserved', code: 'correct-sale' },
        { from: 'sold', to: 'archived', code: 'archive' },
      ],
    },
  },
});
const title: Attribute = { code: 'title', name: 'Title', value_type: 'string' };
const price: Attribute = { code: 'price', name: 'Price', value_type: 'number' };

const edge = (to: string, allowed: boolean): StatusTransitionAccess => ({
  attribute_code: 'sale_status',
  from: 'sold',
  to,
  code: null,
  allowed,
  denial_code: null,
  denial_reason: null,
});

const renderNotice = (
  status: Attribute,
  value: string,
  transitions: StatusTransitionAccess[],
  capabilities: { records_unlock?: boolean } = {},
) => {
  const attributes = [status, title, price];
  const client = new QueryClient({
    defaultOptions: { queries: { staleTime: Infinity } },
  });
  client.setQueryData(authQueryKeys.session(), {
    user_id: 'user-1',
    workspace_id: 'workspace-1',
    capabilities,
  });
  return render(
    <QueryClientProvider client={client}>
      <StatusLockNotice
        attributes={attributes}
        contextId={null}
        locks={savedStatusLocks(
          attributes,
          [
            {
              kind: 'scalar',
              attribute_code: 'sale_status',
              context_id: 'default',
              value,
            },
          ],
          'default',
          [],
        )}
        recordId={recordId}
        transitions={transitions}
        updatedAt={updatedAt}
      />
    </QueryClientProvider>,
  );
};

const notice = (...args: Parameters<typeof renderNotice>) =>
  renderNotice(...args).container.textContent;

afterEach(() => {
  cleanup();
  vi.mocked(unlockRecordStatus).mockReset();
});

describe('StatusLockNotice', () => {
  it('names the locking status and the transitions the user can make', () => {
    expect(
      notice(saleStatus('all'), 'sold', [
        edge('reserved', true),
        edge('archived', true),
      ]),
    ).toBe(
      "Fields can't be changed while Sale status is Sold. To unlock, first change Sale status to Reserved or Archived.",
    );
  });

  it('names the fields a partial lock covers', () => {
    expect(
      notice(saleStatus(['price']), 'sold', [edge('reserved', true)]),
    ).toBe(
      "Price can't be changed while Sale status is Sold. To unlock, first change Sale status to Reserved.",
    );
  });

  it('says when no unlocking transition is available to the user', () => {
    expect(notice(saleStatus('all'), 'sold', [edge('reserved', false)])).toBe(
      "Fields can't be changed while Sale status is Sold. You can't make any of the transitions that unlock it.",
    );
    expect(notice(saleStatus('all'), 'archived', [])).toBe(
      "Fields can't be changed while Sale status is Archived. No transition leaves this status, so the lock is permanent.",
    );
  });

  it('renders nothing when no status locks the record', () => {
    expect(notice(saleStatus('all'), 'reserved', [])).toBe('');
  });

  it('lets owners unlock to any status, outside the declared transitions', async () => {
    vi.mocked(unlockRecordStatus).mockResolvedValue(
      {} as Awaited<ReturnType<typeof unlockRecordStatus>>,
    );
    renderNotice(saleStatus('all'), 'archived', [], { records_unlock: true });
    expect(screen.getByRole('alert').textContent).toBe(
      "Fields can't be changed while Sale status is Archived.Unlock…",
    );
    await userEvent.click(screen.getByRole('button', { name: 'Unlock…' }));
    await userEvent.click(screen.getByRole('button', { name: 'Unlock' }));
    await waitFor(() =>
      expect(unlockRecordStatus).toHaveBeenCalledWith(recordId, {
        attributeCode: 'sale_status',
        contextId: null,
        to: 'reserved',
        expectedUpdatedAt: updatedAt,
      }),
    );
  });

  it('offers no unlock without the permission', () => {
    renderNotice(saleStatus('all'), 'archived', []);
    expect(screen.queryByRole('button')).toBeNull();
  });
});
