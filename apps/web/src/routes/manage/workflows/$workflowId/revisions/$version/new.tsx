import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { WorkflowEditorPage } from '../../../../../../features/workflows/WorkflowEditorPage';
import { RequireCapability } from '../../../../../../features/auth/RequireCapability';
import { capabilities } from '../../../../../../features/auth/capabilities';

export const Route = createFileRoute(
  '/manage/workflows/$workflowId/revisions/$version/new',
)({
  component: WorkflowRevisionEditorRoute,
});

function WorkflowRevisionEditorRoute() {
  const { workflowId, version } = Route.useParams();
  return (
    <RequireCapability capability={capabilities.workflowsManage}>
      <WorkflowEditorPage
        sourceVersion={z.coerce.number().int().positive().parse(version)}
        workflowId={workflowId}
      />
    </RequireCapability>
  );
}
