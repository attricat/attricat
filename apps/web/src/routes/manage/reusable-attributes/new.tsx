import { createFileRoute } from '@tanstack/react-router';
import { ReusableAttributeEditorPage } from '../../../features/reusable-attributes/ReusableAttributeEditorPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';

const NewReusableAttributeRoute = () => (
  <RequireCapability capability="blueprints_write">
    <ReusableAttributeEditorPage />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/reusable-attributes/new')({
  component: NewReusableAttributeRoute,
});
