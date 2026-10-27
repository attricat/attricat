import type { AuditEvent } from './api';
import { personName } from '../../components/personName';

export const auditActor = (event: AuditEvent, systemLabel: string) =>
  personName(
    event.actor_display_name,
    event.actor_email ?? event.actor_user_id ?? systemLabel,
  );

export const auditTarget = (event: AuditEvent, workspaceLabel: string) =>
  Object.entries(event.target)
    .map(([key, value]) => `${key}: ${String(value)}`)
    .join(', ') || workspaceLabel;

/** The actor's name and avatar, or `null` for system events without a user. */
export const auditActorPerson = (event: AuditEvent) =>
  event.actor_user_id
    ? {
        name: personName(
          event.actor_display_name,
          event.actor_email ?? event.actor_user_id,
        ),
        avatarFileId: event.actor_avatar_file_id,
        userId: event.actor_user_id,
      }
    : null;
