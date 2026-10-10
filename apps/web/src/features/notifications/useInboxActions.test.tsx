// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { useToastStore } from '../../components/toastStore';
import { deleteNotification, setNotificationRead } from './api';
import { notificationQueryKeys } from './queryKeys';
import { useInboxActions } from './useInboxActions';

vi.mock('./api', () => ({
  deleteNotification: vi.fn(),
  markAllNotificationsRead: vi.fn(),
  setNotificationRead: vi.fn(),
}));

const workspaceId = '00000000-0000-4000-8000-000000000021';
const userId = '00000000-0000-4000-8000-000000000020';

const deferred = () => {
  let resolve!: () => void;
  let reject!: (cause: Error) => void;
  const promise = new Promise<undefined>((onResolve, onReject) => {
    resolve = () => onResolve(undefined);
    reject = onReject;
  });
  return { promise, reject, resolve };
};

const renderActions = () => {
  const client = new QueryClient();
  const invalidate = vi.spyOn(client, 'invalidateQueries');
  const view = renderHook(() => useInboxActions(workspaceId, userId), {
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    ),
  });
  return { ...view, invalidate };
};

const toastMessages = () =>
  useToastStore.getState().toasts.map(({ message, severity }) => ({
    message,
    severity,
  }));

describe('useInboxActions', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    useToastStore.setState({ toasts: [] });
  });

  it('keeps every changed notification busy until the inbox refreshes', async () => {
    const first = deferred();
    const second = deferred();
    vi.mocked(setNotificationRead).mockReturnValueOnce(first.promise);
    vi.mocked(deleteNotification).mockReturnValueOnce(second.promise);
    const { invalidate, result } = renderActions();

    act(() => {
      result.current.setRead.mutate({ id: 'a', read: true });
      result.current.remove.mutate('b');
    });
    await waitFor(() => expect(result.current.isBusy('a')).toBe(true));
    expect(result.current.isBusy('b')).toBe(true);
    expect(result.current.isBusy('c')).toBe(false);

    await act(async () => first.resolve());
    await waitFor(() => expect(result.current.isBusy('a')).toBe(false));
    expect(result.current.isBusy('b')).toBe(true);
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: notificationQueryKeys.all(workspaceId, userId),
    });

    await act(async () => second.resolve());
    await waitFor(() => expect(result.current.isBusy('b')).toBe(false));
    expect(toastMessages()).toEqual([
      { message: 'Notification deleted.', severity: 'success' },
    ]);
  });

  it('reports a failed change and frees the notification', async () => {
    vi.mocked(setNotificationRead).mockRejectedValueOnce(
      new Error('Notification not found'),
    );
    const { invalidate, result } = renderActions();

    act(() => result.current.setRead.mutate({ id: 'a', read: false }));
    await waitFor(() => expect(result.current.setRead.isError).toBe(true));
    await waitFor(() => expect(result.current.isBusy('a')).toBe(false));
    expect(invalidate).toHaveBeenCalledOnce();
    expect(toastMessages()).toEqual([
      { message: 'Notification not found', severity: 'error' },
    ]);
  });
});
