import { createFileRoute } from '@tanstack/react-router';
import { WorkspaceManagementPage } from '../../../features/workspace/WorkspaceManagementPage';

export const Route = createFileRoute('/manage/workspace/roles')({
  component: () => <WorkspaceManagementPage section="roles" />,
});
