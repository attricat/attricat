import { createFileRoute } from '@tanstack/react-router';
import { WorkflowsPage } from '../../../features/workflows/WorkflowsPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';
import { capabilities } from '../../../features/auth/capabilities';

const WorkflowsRoute = () => (
  <RequireCapability capability={capabilities.workflowsRead}>
    <WorkflowsPage />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/workflows/')({
  component: WorkflowsRoute,
});
