import { createFileRoute } from '@tanstack/react-router';
import { WorkflowDetailPage } from '../../../../features/workflows/WorkflowDetailPage';

export const Route = createFileRoute('/manage/workflows/$workflowId/')({
  component: WorkflowDetailRoute,
});

function WorkflowDetailRoute() {
  return <WorkflowDetailPage workflowId={Route.useParams().workflowId} />;
}
