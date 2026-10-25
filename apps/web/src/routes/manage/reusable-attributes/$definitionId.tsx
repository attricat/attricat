import { createFileRoute } from '@tanstack/react-router';
import { ReusableAttributeEditorPage } from '../../../features/reusable-attributes/ReusableAttributeEditorPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';
import { capabilities } from '../../../features/auth/capabilities';

const ReusableAttributeEditorRoute = () => (
  <RequireCapability capability={capabilities.blueprintsWrite}>
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
