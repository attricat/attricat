import { createFileRoute } from '@tanstack/react-router';
import { SystemHealthPage } from '../../features/system-health/SystemHealthPage';

export const Route = createFileRoute('/manage/system-health')({
  component: SystemHealthPage,
});
