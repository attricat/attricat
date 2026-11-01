import { createFileRoute } from '@tanstack/react-router';
import { WorkspaceManagementPage } from '../../../features/workspace/WorkspaceManagementPage';

export const Route = createFileRoute('/manage/workspace/sign-in')({
  component: () => <WorkspaceManagementPage section="sign-in" />,
});
