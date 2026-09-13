import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { WorkflowEditorPage } from '../../../../../../features/workflows/WorkflowEditorPage';

export const Route = createFileRoute(
  '/manage/workflows/$workflowId/revisions/$version/new',
)({
  component: WorkflowRevisionEditorRoute,
});

function WorkflowRevisionEditorRoute() {
  const { workflowId, version } = Route.useParams();
  return (
    <WorkflowEditorPage
      sourceVersion={z.coerce.number().int().positive().parse(version)}
      workflowId={workflowId}
    />
  );
}
