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
