import { createFileRoute } from '@tanstack/react-router';
import { ExportsPage } from '../../features/exports/ExportsPage';

export const Route = createFileRoute('/manage/exports')({
  component: ExportsPage,
});
