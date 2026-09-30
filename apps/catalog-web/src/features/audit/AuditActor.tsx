import { UserLabel } from '../../components/UserLabel';
import type { AuditEvent } from './api';
import { auditActorPerson } from './auditFormat';

/** The event's human actor with their avatar, or the system label. */
export const AuditActor = ({
  event,
  systemLabel,
}: {
  event: AuditEvent;
  systemLabel: string;
}) => {
  const person = auditActorPerson(event);
  return person ? <UserLabel {...person} /> : systemLabel;
};
