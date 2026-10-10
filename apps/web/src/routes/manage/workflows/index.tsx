import { createFileRoute } from '@tanstack/react-router';
import { WorkflowsPage } from '../../../features/workflows/WorkflowsPage';

export const Route = createFileRoute('/manage/workflows/')({
  component: WorkflowsPage,
});
