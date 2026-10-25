import { createFileRoute } from '@tanstack/react-router';
import { BlueprintEditorPage } from '../../../features/blueprints/BlueprintEditorPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';
import { capabilities } from '../../../features/auth/capabilities';

const NewBlueprintRoute = () => (
  <RequireCapability capability={capabilities.blueprintsWrite}>
    <BlueprintEditorPage />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/blueprints/new')({
  component: NewBlueprintRoute,
});
