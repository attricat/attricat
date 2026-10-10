import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { CreateRecordPage } from '../../features/records/CreateRecordPage';

const NewRecordRouteComponent = () => (
  <CreateRecordPage search={Route.useSearch()} />
);

export const Route = createFileRoute('/records/new')({
  validateSearch: z.object({
    blueprint: z.string().trim().min(1).optional().catch(undefined),
    locked: z.boolean().optional().catch(undefined),
  }),
  component: NewRecordRouteComponent,
});
