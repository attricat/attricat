import type { AuditEvent } from './api';
import { auditDateTimeFormat } from './constants';

export const formatAuditDate = (value: string, locale: string) =>
  new Intl.DateTimeFormat(locale, auditDateTimeFormat).format(new Date(value));

export const auditActor = (event: AuditEvent, systemLabel: string) =>
  event.actor_display_name ??
  event.actor_email ??
  event.actor_user_id ??
  systemLabel;

export const auditTarget = (event: AuditEvent, workspaceLabel: string) =>
  Object.entries(event.target)
    .map(([key, value]) => `${key}: ${String(value)}`)
    .join(', ') || workspaceLabel;

export const localDateTimeToIso = (value: string) =>
  value ? new Date(value).toISOString() : '';
