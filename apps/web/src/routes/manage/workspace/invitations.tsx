import { createFileRoute } from '@tanstack/react-router';
import { WorkspaceManagementPage } from '../../../features/workspace/WorkspaceManagementPage';

export const Route = createFileRoute('/manage/workspace/invitations')({
  component: () => <WorkspaceManagementPage section="invitations" />,
});
