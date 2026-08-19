import { createFileRoute } from '@tanstack/react-router';
import { BlueprintsPage } from '../../features/blueprints/BlueprintsPage';

export const Route = createFileRoute('/blueprints/')({
  component: BlueprintsPage,
});
