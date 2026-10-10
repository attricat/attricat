import { createFileRoute } from '@tanstack/react-router';
import { CreateContextPage } from '../../../features/contexts/CreateContextPage';

export const Route = createFileRoute('/manage/contexts/new')({
  component: CreateContextPage,
});
