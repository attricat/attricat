import { createFileRoute } from '@tanstack/react-router';
import { AcceptInvitationPage } from '../../features/workspace/WorkspaceManagementPage';

export const Route = createFileRoute('/invitations/accept')({
  component: AcceptInvitationPage,
});
