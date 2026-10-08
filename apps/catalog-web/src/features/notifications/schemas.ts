import { z } from 'zod';

export const notificationSubjectSchema = z.object({
  kind: z.enum(['entity', 'agent_conversation']),
  id: z.uuid(),
});

export const notificationSchema = z.object({
  id: z.uuid(),
  kind: z.string(),
  title: z.string(),
  body: z.string().nullable(),
  actor_user_id: z.uuid().nullable(),
  actor_display_name: z.string().nullable(),
  actor_email: z.string().nullable(),
  subject: notificationSubjectSchema.nullable(),
  data: z.record(z.string(), z.unknown()),
  read: z.boolean(),
  read_at: z.string().datetime().nullable(),
  created_at: z.string().datetime(),
});

export const notificationPageSchema = z.object({
  items: z.array(notificationSchema),
  has_more: z.boolean(),
  unread_count: z.number().int().nonnegative(),
});

export const unreadCountSchema = z.object({
  count: z.number().int().nonnegative(),
});

export const markAllReadResultSchema = z.object({
  updated: z.number().int().nonnegative(),
});

export type Notification = z.infer<typeof notificationSchema>;
export type NotificationPage = z.infer<typeof notificationPageSchema>;
export type NotificationCursor = { before_time: string; before_id: string };
