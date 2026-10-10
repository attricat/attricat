import type { AuditEvent } from './api';

export const auditActor = (event: AuditEvent, systemLabel: string) =>
  event.actor_display_name ??
  event.actor_email ??
  event.actor_user_id ??
  systemLabel;

export const auditTarget = (event: AuditEvent, workspaceLabel: string) =>
  Object.entries(event.target)
    .map(([key, value]) => `${key}: ${String(value)}`)
    .join(', ') || workspaceLabel;

/** The actor's name and avatar, or `null` for system events without a user. */
export const auditActorPerson = (event: AuditEvent) =>
  event.actor_user_id
    ? {
        name:
          event.actor_display_name ?? event.actor_email ?? event.actor_user_id,
        avatarFileId: event.actor_avatar_file_id,
      }
    : null;
