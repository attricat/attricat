import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { AcceptInvitationPage } from '../../features/workspace/AcceptInvitationPage';

const AcceptInvitationRoute = () => (
  <AcceptInvitationPage secret={Route.useSearch().secret} />
);

export const Route = createFileRoute('/invitations/accept')({
  validateSearch: z.object({ secret: z.string().optional() }),
  component: AcceptInvitationRoute,
});
