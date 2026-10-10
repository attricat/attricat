import { useQuery } from '@tanstack/react-query';
import { getUnreadCount } from './api';
import { unreadCountRefreshMs } from './constants';
import { notificationQueryKeys } from './queryKeys';
import { useInboxIdentity } from './useInboxIdentity';

/** The current member's unread notification count, refreshed periodically. */
export const useUnreadNotificationCount = () => {
  const { workspaceId, userId } = useInboxIdentity();
  const unread = useQuery({
    queryKey: notificationQueryKeys.unreadCount(workspaceId, userId),
    queryFn: ({ signal }) => getUnreadCount(signal),
    enabled: Boolean(workspaceId && userId),
    refetchInterval: unreadCountRefreshMs,
  });
  return unread.data?.count ?? 0;
};
