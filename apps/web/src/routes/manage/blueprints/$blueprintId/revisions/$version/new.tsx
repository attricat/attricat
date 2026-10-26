import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { BlueprintEditorPage } from '../../../../../../features/blueprints/BlueprintEditorPage';
import { RequireCapability } from '../../../../../../features/auth/RequireCapability';
import {
  capabilities,
  accessTargetKinds,
} from '../../../../../../features/auth/capabilities';

const RevisionEditorRoute = () => {
  const { blueprintId, version } = Route.useParams();
  return (
    <RequireCapability
      capability={capabilities.blueprintsWrite}
      target={{ kind: accessTargetKinds.blueprint, id: blueprintId }}
    >
      <BlueprintEditorPage
        blueprintId={blueprintId}
        sourceVersion={z.coerce.number().int().positive().parse(version)}
      />
    </RequireCapability>
  );
};

export const Route = createFileRoute(
  '/manage/blueprints/$blueprintId/revisions/$version/new',
)({
  component: RevisionEditorRoute,
});
