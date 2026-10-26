import { createFileRoute } from '@tanstack/react-router';
import { WorkflowEditorPage } from '../../../features/workflows/WorkflowEditorPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';
import { capabilities } from '../../../features/auth/capabilities';

const NewWorkflowRoute = () => (
  <RequireCapability capability={capabilities.workflowsManage}>
    <WorkflowEditorPage />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/workflows/new')({
  component: NewWorkflowRoute,
});
