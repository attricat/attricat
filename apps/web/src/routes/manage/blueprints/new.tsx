import { createFileRoute } from '@tanstack/react-router';
import { BlueprintEditorPage } from '../../../features/blueprints/BlueprintEditorPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';

const NewBlueprintRoute = () => (
  <RequireCapability capability="blueprints_write">
    <BlueprintEditorPage />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/blueprints/new')({
  component: NewBlueprintRoute,
});
