// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { HandlerFailure } from '../extension-exports/schemas';
import { listDeliveryFailures, listRunFailures } from './api';
import { ExtensionFailuresSection } from './ExtensionFailuresSection';
import { runFailureStates, type RunFailure } from './schemas';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  listDeliveryFailures: vi.fn(),
  listRunFailures: vi.fn(),
}));

const extensionId = 'acme.invoices';
const run = (overrides: Partial<RunFailure> = {}): RunFailure => ({
  id: '00000000-0000-4000-8000-000000000001',
  operation_id: 'issue-invoices',
  state: runFailureStates.failed,
  scheduled: false,
  attempts: 1,
  error_code: 'client_rejected',
  error_message: 'NIP is invalid',
  updated_at: '2026-11-28T10:00:00Z',
  ...overrides,
});
const delivery: HandlerFailure = {
  event_id: '00000000-0000-4000-8000-000000000002',
  event_type: 'record.updated.v1',
  handler_id: 'push-client',
  status: 'rejected',
  attempts: 1,
  error_code: 'client_rejected',
  error_message: 'Email is taken',
  record_id: null,
  updated_at: '2026-11-28T09:00:00Z',
};

const renderSection = () =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ExtensionFailuresSection extensionId={extensionId} />
    </QueryClientProvider>,
  );

describe('ExtensionFailuresSection', () => {
  beforeEach(() => {
    vi.mocked(listRunFailures).mockReset();
    vi.mocked(listDeliveryFailures).mockReset();
  });

  it("shows each failure with the extension's own code and message", async () => {
    vi.mocked(listRunFailures).mockResolvedValue({
      items: [
        run(),
        run({
          id: '00000000-0000-4000-8000-000000000003',
          operation_id: 'pull',
          state: runFailureStates.retrying,
          scheduled: true,
          error_code: 'retry',
          error_message: 'Fakturownia is unavailable',
        }),
      ],
      has_more: false,
    });
    vi.mocked(listDeliveryFailures).mockResolvedValue({
      items: [delivery],
      has_more: false,
    });
    renderSection();

    expect(
      await screen.findByText('client_rejected: NIP is invalid'),
    ).toBeTruthy();
    expect(screen.getByText('retry: Fakturownia is unavailable')).toBeTruthy();
    expect(screen.getByText('Started by a schedule')).toBeTruthy();
    expect(screen.getByText('Retrying')).toBeTruthy();
    expect(screen.getByText('client_rejected: Email is taken')).toBeTruthy();
    expect(screen.queryByRole('button', { name: /load more/i })).toBeNull();
  });

  it('loads the next page of runs before the last one shown', async () => {
    vi.mocked(listRunFailures)
      .mockResolvedValueOnce({ items: [run()], has_more: true })
      .mockResolvedValueOnce({
        items: [
          run({
            id: '00000000-0000-4000-8000-000000000004',
            error_message: 'An older failure',
            updated_at: '2026-11-27T10:00:00Z',
          }),
        ],
        has_more: false,
      });
    vi.mocked(listDeliveryFailures).mockResolvedValue({
      items: [],
      has_more: false,
    });
    renderSection();

    await userEvent.click(
      await screen.findByRole('button', { name: /load more/i }),
    );
    expect(
      await screen.findByText('client_rejected: An older failure'),
    ).toBeTruthy();
    expect(listRunFailures).toHaveBeenLastCalledWith(extensionId, {
      before_time: '2026-11-28T10:00:00Z',
      before_id: '00000000-0000-4000-8000-000000000001',
    });
  });

  it('says so when nothing failed', async () => {
    vi.mocked(listRunFailures).mockResolvedValue({
      items: [],
      has_more: false,
    });
    vi.mocked(listDeliveryFailures).mockResolvedValue({
      items: [],
      has_more: false,
    });
    renderSection();

    expect(await screen.findByText('No failures')).toBeTruthy();
  });
});
