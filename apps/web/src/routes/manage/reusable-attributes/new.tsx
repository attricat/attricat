import { createFileRoute } from '@tanstack/react-router';
import { ReusableAttributeEditorPage } from '../../../features/reusable-attributes/ReusableAttributeEditorPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';
import { capabilities } from '../../../features/auth/capabilities';

const NewReusableAttributeRoute = () => (
  <RequireCapability capability={capabilities.blueprintsWrite}>
    <ReusableAttributeEditorPage />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/reusable-attributes/new')({
  component: NewReusableAttributeRoute,
});
