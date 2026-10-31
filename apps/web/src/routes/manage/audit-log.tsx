import { createFileRoute } from '@tanstack/react-router';
import { AuditLogPage } from '../../features/audit/AuditLogPage';
import { RequireCapability } from '../../features/auth/RequireCapability';
import { capabilities } from '../../features/auth/capabilities';

const AuditLogRoute = () => (
  <RequireCapability capability={capabilities.auditRead}>
    <AuditLogPage />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/audit-log')({
  component: AuditLogRoute,
});
