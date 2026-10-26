import { request, requestNoContent, jsonRequest } from '../../api/request';
import { notificationsPath } from './constants';
import {
  markAllReadResultSchema,
  notificationPageSchema,
  unreadCountSchema,
} from './schemas';
import type { KeysetCursor } from '../../api/keysetCursor';

const notificationPath = (id: string) =>
  `${notificationsPath}/${encodeURIComponent(id)}`;

export const listNotifications = (
  unreadOnly: boolean,
  cursor?: KeysetCursor,
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
  requestNoContent(notificationPath(id), jsonRequest('PATCH', { read }));

export const markAllNotificationsRead = (upTo?: string) =>
  request(
    `${notificationsPath}/read-all`,
    markAllReadResultSchema,
    jsonRequest('POST', upTo ? { up_to: upTo } : {}),
  );

export const deleteNotification = (id: string) =>
  requestNoContent(notificationPath(id), { method: 'DELETE' });
