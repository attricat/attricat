import { createFileRoute } from '@tanstack/react-router';
import { ContextsPage } from '../../../features/contexts/ContextsPage';

export const Route = createFileRoute('/manage/contexts/')({
  component: ContextsPage,
});
