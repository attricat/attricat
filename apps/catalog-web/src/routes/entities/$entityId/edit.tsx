import { createFileRoute } from '@tanstack/react-router';
import { EditEntityPage } from '../../../features/entities/EditEntityPage';

const EditEntityRouteComponent = () => (
  <EditEntityPage entityId={Route.useParams().entityId} />
);

export const Route = createFileRoute('/entities/$entityId/edit')({
  component: EditEntityRouteComponent,
});
