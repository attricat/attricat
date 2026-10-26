import { createFileRoute } from '@tanstack/react-router';
import { WorkflowDetailPage } from '../../../../features/workflows/WorkflowDetailPage';
import { RequireCapability } from '../../../../features/auth/RequireCapability';
import { capabilities } from '../../../../features/auth/capabilities';

export const Route = createFileRoute('/manage/workflows/$workflowId/')({
  component: WorkflowDetailRoute,
});

function WorkflowDetailRoute() {
  return (
    <RequireCapability capability={capabilities.workflowsRead}>
      <WorkflowDetailPage workflowId={Route.useParams().workflowId} />
    </RequireCapability>
  );
}
