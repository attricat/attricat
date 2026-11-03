// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { httpStatus } from '../../api/httpStatus';
import { ApiRequestError } from '../../api/request';
import { getExtensionRun } from './api';
import { extensionRunStatuses } from './constants';
import { ExtensionRunWatcher } from './ExtensionRunWatcher';
import { extensionRunQueryKeys } from './queryKeys';
import { notifyRunFinished } from './runNotifications';
import { useRunWatchStore } from './runWatchStore';

vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('./api', () => ({ getExtensionRun: vi.fn() }));
vi.mock('./runNotifications', () => ({ notifyRunFinished: vi.fn() }));

const run = (id: string, status: string) =>
  ({ id, scope: 'record', status }) as never;

const renderWatcher = (runId: string) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <ExtensionRunWatcher />
    </QueryClientProvider>,
  );
  act(() => useRunWatchStore.getState().watch(runId));
  const refetch = () =>
    act(() =>
      client.refetchQueries({ queryKey: extensionRunQueryKeys.detail(runId) }),
    );
  return { refetch };
};

describe('ExtensionRunWatcher', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useRunWatchStore.setState({ runIds: [], records: {} });
  });

  it('keeps watching through a brief outage and announces the finish', async () => {
    vi.mocked(getExtensionRun).mockResolvedValueOnce(
      run('66666666-6666-4666-8666-666666666666', extensionRunStatuses.running),
    );
    const { refetch } = renderWatcher('66666666-6666-4666-8666-666666666666');
    await waitFor(() => expect(getExtensionRun).toHaveBeenCalledTimes(1));

    vi.mocked(getExtensionRun).mockRejectedValueOnce(new TypeError('offline'));
    await refetch();
    expect(useRunWatchStore.getState().runIds).toEqual([
      '66666666-6666-4666-8666-666666666666',
    ]);

    vi.mocked(getExtensionRun).mockResolvedValueOnce(
      run(
        '66666666-6666-4666-8666-666666666666',
        extensionRunStatuses.completed,
      ),
    );
    await refetch();
    await waitFor(() => expect(notifyRunFinished).toHaveBeenCalledTimes(1));
    expect(useRunWatchStore.getState().runIds).toEqual([]);
  });

  it('stops watching a run the server refuses', async () => {
    vi.mocked(getExtensionRun).mockRejectedValue(
      new ApiRequestError(httpStatus.notFound, 'missing'),
    );
    renderWatcher('missing');

    await waitFor(() => expect(useRunWatchStore.getState().runIds).toEqual([]));
    expect(notifyRunFinished).not.toHaveBeenCalled();
  });
});
