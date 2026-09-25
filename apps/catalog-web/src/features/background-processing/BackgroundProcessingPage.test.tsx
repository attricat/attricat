// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import i18n from '../../i18n';
import { currentSession } from '../auth/api';
import { getBackgroundProcessingStatus } from './api';
import { BackgroundProcessingPage } from './BackgroundProcessingPage';

vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./api', () => ({ getBackgroundProcessingStatus: vi.fn() }));

const session = (canRead: boolean) =>
  ({
    workspace_id: 'workspace-one',
    capabilities: { data_health_read: canRead },
  }) as Awaited<ReturnType<typeof currentSession>>;
const renderPage = () => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <BackgroundProcessingPage />
    </QueryClientProvider>,
  );
  return client;
};

beforeEach(async () => {
  await i18n.changeLanguage('en');
  vi.mocked(currentSession).mockResolvedValue(session(true));
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe('BackgroundProcessingPage', () => {
  it('does not request queue data without permission', async () => {
    vi.mocked(currentSession).mockResolvedValue(session(false));
    renderPage();
    expect(
      await screen.findByText(
        'You are not authorized to view background processing in this workspace.',
      ),
    ).toBeTruthy();
    expect(getBackgroundProcessingStatus).not.toHaveBeenCalled();
  });

  it('distinguishes loading from an empty queue', async () => {
    let finish!: (rows: []) => void;
    vi.mocked(getBackgroundProcessingStatus).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    renderPage();
    await waitFor(() =>
      expect(getBackgroundProcessingStatus).toHaveBeenCalledOnce(),
    );
    expect(screen.getByRole('progressbar')).toBeTruthy();
    expect(
      screen.queryByText(
        'No queued, running, or failed API tasks in this workspace.',
      ),
    ).toBeNull();
    finish([]);
    expect(
      await screen.findByText(
        'No queued, running, or failed API tasks in this workspace.',
      ),
    ).toBeTruthy();
    expect(
      screen.getByText(/File processing is a separate queue/),
    ).toBeTruthy();
  });

  it('shows safe errors and supports retrying a failed request', async () => {
    vi.mocked(getBackgroundProcessingStatus)
      .mockRejectedValueOnce(new Error('secret database exception'))
      .mockResolvedValue([]);
    renderPage();
    expect(
      await screen.findByText(
        'Could not load background processing status. Try refreshing.',
      ),
    ).toBeTruthy();
    expect(screen.queryByText(/secret database/)).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    expect(
      await screen.findByText(
        'No queued, running, or failed API tasks in this workspace.',
      ),
    ).toBeTruthy();
  });

  it('shows counts, failed-task and expired-lease warnings and marks stale data after refresh failure', async () => {
    vi.mocked(getBackgroundProcessingStatus)
      .mockResolvedValueOnce([
        {
          kind: 'rule_run.v1',
          queued: 12,
          running: 4,
          failed: 3,
          expired_leases: 2,
          oldest_due_seconds: 120,
        },
      ])
      .mockRejectedValue(new Error('secret refresh exception'));
    renderPage();
    expect(
      await screen.findByRole('table', { name: 'Background processing' }),
    ).toBeTruthy();
    expect(screen.getByRole('rowheader', { name: 'Rule runs' })).toBeTruthy();
    for (const value of ['12', '4', '3', '2', '120 s'])
      expect(screen.getByRole('cell', { name: value })).toBeTruthy();
    expect(
      screen.getByText(/Some tasks failed and will not retry automatically/),
    ).toBeTruthy();
    expect(
      screen.getByText(/Some running tasks have expired leases/),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    expect(
      await screen.findByText(
        'Refresh failed. The displayed status may be outdated.',
      ),
    ).toBeTruthy();
    expect(screen.getByRole('table')).toBeTruthy();
    expect(screen.queryByText(/secret refresh/)).toBeNull();
  });
});
