import { createFileRoute } from '@tanstack/react-router';
import { MigrateEntityPage } from '../../../features/entities/MigrateEntityPage';

const MigrateEntityRouteComponent = () => (
  <MigrateEntityPage entityId={Route.useParams().entityId} />
);

export const Route = createFileRoute('/entities/$entityId/migrate')({
  component: MigrateEntityRouteComponent,
});
