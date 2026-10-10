import { request, requestNoContent } from '../../api/request';
import { notificationsPath } from './constants';
import {
  markAllReadResultSchema,
  notificationPageSchema,
  unreadCountSchema,
  type NotificationCursor,
} from './schemas';

const notificationPath = (id: string) =>
  `${notificationsPath}/${encodeURIComponent(id)}`;

export const listNotifications = (
  unreadOnly: boolean,
  cursor?: NotificationCursor,
  signal?: AbortSignal,
) => {
  const params = new URLSearchParams(cursor);
  if (unreadOnly) params.set('unread_only', 'true');
  const query = params.size > 0 ? `?${params}` : '';
  return request(`${notificationsPath}${query}`, notificationPageSchema, {
    signal,
  });
};

export const getUnreadCount = (signal?: AbortSignal) =>
  request(`${notificationsPath}/unread-count`, unreadCountSchema, { signal });

export const setNotificationRead = (id: string, read: boolean) =>
  requestNoContent(notificationPath(id), {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ read }),
  });

export const markAllNotificationsRead = (upTo?: string) =>
  request(`${notificationsPath}/read-all`, markAllReadResultSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(upTo ? { up_to: upTo } : {}),
  });

export const deleteNotification = (id: string) =>
  requestNoContent(notificationPath(id), { method: 'DELETE' });
