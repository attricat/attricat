import { createFileRoute } from '@tanstack/react-router';
import { EntityChangesPage } from '../../../features/entities/EntityChangesPage';

const EntityChangesRouteComponent = () => (
  <EntityChangesPage entityId={Route.useParams().entityId} />
);

export const Route = createFileRoute('/entities/$entityId/changes')({
  component: EntityChangesRouteComponent,
});
