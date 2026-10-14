import { createFileRoute } from '@tanstack/react-router';
import { BlueprintEditorPage } from '../../../../../../features/blueprints/BlueprintEditorPage';
import { RequireCapability } from '../../../../../../features/auth/RequireCapability';

const RevisionEditorRoute = () => {
  const { blueprintId, version } = Route.useParams();
  return (
    <RequireCapability
      capability="blueprints_write"
      target={{ kind: 'blueprint', id: blueprintId }}
    >
      <BlueprintEditorPage
        blueprintId={blueprintId}
        sourceVersion={Number(version)}
      />
    </RequireCapability>
  );
};

export const Route = createFileRoute(
  '/manage/blueprints/$blueprintId/revisions/$version/new',
)({
  component: RevisionEditorRoute,
});
