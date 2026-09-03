import { createFileRoute } from '@tanstack/react-router';
import { BlueprintsPage } from '../../../features/blueprints/BlueprintsPage';

export const Route = createFileRoute('/manage/blueprints/')({
  component: BlueprintsPage,
});
