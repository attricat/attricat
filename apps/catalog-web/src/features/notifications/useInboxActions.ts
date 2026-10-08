import { useMutation, useQueryClient } from '@tanstack/react-query';
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

/** Inbox changes; each refreshes the inbox lists and the unread count. */
export const useInboxActions = (
  workspaceId: string | undefined,
  userId: string | undefined,
) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const { show } = useToast();
  const onSettled = () =>
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
    onError,
    onSettled,
  });
  const markAllRead = useMutation({
    mutationFn: (upTo: string | undefined) => markAllNotificationsRead(upTo),
    meta,
    onError,
    onSettled,
  });
  const remove = useMutation({
    mutationFn: (id: string) => deleteNotification(id),
    meta,
    onError,
    onSuccess: () => show({ message: t('inbox.deleted'), severity: 'success' }),
    onSettled,
  });
  return { setRead, markAllRead, remove };
};
