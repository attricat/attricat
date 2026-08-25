import { createFileRoute } from '@tanstack/react-router';
import { WorkspaceManagementPage } from '../../features/workspace/WorkspaceManagementPage';

export const Route = createFileRoute('/workspace/tokens')({
  component: () => <WorkspaceManagementPage section="tokens" />,
});
