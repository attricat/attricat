import { createFileRoute } from '@tanstack/react-router';
import { ReusableAttributeEditorPage } from '../../../features/reusable-attributes/ReusableAttributeEditorPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';

const ReusableAttributeEditorRoute = () => (
  <RequireCapability capability="blueprints_write">
    <ReusableAttributeEditorPage
      definitionId={Route.useParams().definitionId}
    />
  </RequireCapability>
);

export const Route = createFileRoute(
  '/manage/reusable-attributes/$definitionId',
)({
  component: ReusableAttributeEditorRoute,
});
