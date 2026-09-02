import { createFileRoute } from '@tanstack/react-router';
import { SchedulesPage } from '../../features/agents/SchedulesPage';

export const Route = createFileRoute('/agents/schedules')({
  component: SchedulesPage,
});
