import { createFileRoute } from '@tanstack/react-router';
import { WorkflowEditorPage } from '../../../features/workflows/WorkflowEditorPage';

export const Route = createFileRoute('/manage/workflows/new')({
  component: WorkflowEditorPage,
});
