import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { CreateRecordPage } from '../../features/records/CreateRecordPage';
import { RequireCapability } from '../../features/auth/RequireCapability';

const NewRecordRouteComponent = () => (
  <RequireCapability capability="records_write">
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
