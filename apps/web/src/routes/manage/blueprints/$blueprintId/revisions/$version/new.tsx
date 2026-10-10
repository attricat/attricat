import { createFileRoute } from '@tanstack/react-router';
import { BlueprintEditorPage } from '../../../../../../features/blueprints/BlueprintEditorPage';

const RevisionEditorRoute = () => {
  const { blueprintId, version } = Route.useParams();
  return (
    <BlueprintEditorPage
      blueprintId={blueprintId}
      sourceVersion={Number(version)}
    />
  );
};

export const Route = createFileRoute(
  '/manage/blueprints/$blueprintId/revisions/$version/new',
)({
  component: RevisionEditorRoute,
});
