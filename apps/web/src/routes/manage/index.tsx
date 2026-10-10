import { createFileRoute } from '@tanstack/react-router';
import { ManagementDashboardPage } from '../../features/workspace/ManagementDashboardPage';

export const Route = createFileRoute('/manage/')({
  component: ManagementDashboardPage,
});
