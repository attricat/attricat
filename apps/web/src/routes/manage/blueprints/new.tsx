import { createFileRoute } from '@tanstack/react-router';
import { BlueprintEditorPage } from '../../../features/blueprints/BlueprintEditorPage';

export const Route = createFileRoute('/manage/blueprints/new')({
  component: BlueprintEditorPage,
});
