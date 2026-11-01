import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { SingleSignOnCompletePage } from '../../features/auth/SingleSignOnCompletePage';

const SingleSignOnCompleteRoute = () => (
  <SingleSignOnCompletePage error={Route.useSearch().error} />
);

export const Route = createFileRoute('/oidc/complete')({
  validateSearch: z.object({
    error: z.string().optional(),
    linked: z.coerce.boolean().optional(),
  }),
  component: SingleSignOnCompleteRoute,
});
