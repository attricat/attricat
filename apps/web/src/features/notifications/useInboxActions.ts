import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useToast } from '../../components/useToast';
import {
  deleteNotification,
  markAllNotificationsRead,
  setNotificationRead,
} from './api';
import { notificationQueryKeys } from './queryKeys';

const errorMessage = (cause: unknown) =>
  cause instanceof Error ? cause.message : String(cause);

/**
 * Inbox changes; each refreshes the inbox lists and the unread count. A
 * notification stays busy from its change until the refreshed list arrives,
 * so a second change cannot be sent from its stale row.
 */
export const useInboxActions = (
  workspaceId: string | undefined,
  userId: string | undefined,
) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const { show } = useToast();
  const [busyIds, setBusyIds] = useState<ReadonlySet<string>>(new Set());
  const setBusy = (id: string, busy: boolean) =>
    setBusyIds((current) => {
      const next = new Set(current);
      if (busy) next.add(id);
      else next.delete(id);
      return next;
    });
  const refresh = () =>
    client.invalidateQueries({
      queryKey: notificationQueryKeys.all(workspaceId, userId),
    });
  const onError = (cause: unknown) =>
    show({ message: errorMessage(cause), severity: 'error' });
  // The list shows read changes itself; deletion has its own message.
  const meta = { toast: false };
  const setRead = useMutation({
    mutationFn: ({ id, read }: { id: string; read: boolean }) =>
      setNotificationRead(id, read),
    meta,
    onMutate: ({ id }) => setBusy(id, true),
    onError,
    onSettled: async (_, __, { id }) => {
      await refresh();
      setBusy(id, false);
    },
  });
  const markAllRead = useMutation({
    mutationFn: (upTo: string | undefined) => markAllNotificationsRead(upTo),
    meta,
    onError,
    onSettled: refresh,
  });
  const remove = useMutation({
    mutationFn: (id: string) => deleteNotification(id),
    meta,
    onMutate: (id) => setBusy(id, true),
    onError,
    onSuccess: () => show({ message: t('inbox.deleted'), severity: 'success' }),
    onSettled: async (_, __, id) => {
      await refresh();
      setBusy(id, false);
    },
  });
  return {
    isBusy: (id: string) => busyIds.has(id),
    markAllRead,
    remove,
    setRead,
  };
};
