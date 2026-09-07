import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { CreateEntityPage } from '../../features/entities/CreateEntityPage';

const NewEntityRouteComponent = () => (
  <CreateEntityPage search={Route.useSearch()} />
);

export const Route = createFileRoute('/entities/new')({
  validateSearch: z.object({
    blueprint: z.string().trim().min(1).optional().catch(undefined),
    locked: z.boolean().optional().catch(undefined),
  }),
  component: NewEntityRouteComponent,
});
