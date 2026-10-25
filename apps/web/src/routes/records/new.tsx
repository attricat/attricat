import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { CreateRecordPage } from '../../features/records/CreateRecordPage';
import { RequireCapability } from '../../features/auth/RequireCapability';
import { capabilities } from '../../features/auth/capabilities';

const NewRecordRouteComponent = () => (
  <RequireCapability capability={capabilities.recordsWrite}>
    <CreateRecordPage search={Route.useSearch()} />
  </RequireCapability>
);

export const Route = createFileRoute('/records/new')({
  validateSearch: z.object({
    blueprint: z.string().trim().min(1).optional().catch(undefined),
    locked: z.boolean().optional().catch(undefined),
  }),
  component: NewRecordRouteComponent,
});
