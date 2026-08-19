import { createFileRoute } from '@tanstack/react-router';
import { BlueprintDetailPage } from '../../features/blueprints/BlueprintDetailPage';

const BlueprintDetailRoute = () => (
  <BlueprintDetailPage blueprintId={Route.useParams().blueprintId} />
);

export const Route = createFileRoute('/blueprints/$blueprintId')({
  component: BlueprintDetailRoute,
});
