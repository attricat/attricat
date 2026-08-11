import { createFileRoute } from '@tanstack/react-router';
import { EntityPreviewPage } from '../../../features/entities/EntityPreviewPage';

const EntityPreviewRouteComponent = () => (
  <EntityPreviewPage entityId={Route.useParams().entityId} />
);

export const Route = createFileRoute('/entities/$entityId/')({
  component: EntityPreviewRouteComponent,
});
