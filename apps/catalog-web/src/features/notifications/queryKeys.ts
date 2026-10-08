import type { InboxFilter } from './constants';

export const notificationQueryKeys = {
  /** Every notification query of one member's inbox in one workspace. */
  all: (workspaceId: string | undefined, userId: string | undefined) =>
    ['notifications', workspaceId, userId] as const,
  list: (
    workspaceId: string | undefined,
    userId: string | undefined,
    filter: InboxFilter,
  ) =>
    [
      ...notificationQueryKeys.all(workspaceId, userId),
      'list',
      filter,
    ] as const,
  unreadCount: (workspaceId: string | undefined, userId: string | undefined) =>
    [
      ...notificationQueryKeys.all(workspaceId, userId),
      'unread-count',
    ] as const,
};
