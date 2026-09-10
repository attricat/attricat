import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { EntityPreviewPage } from '../../../features/entities/EntityPreviewPage';

const EntityPreviewRouteComponent = () => (
  <EntityPreviewPage
    entityId={Route.useParams().entityId}
    relationshipPickerToken={Route.useSearch().relationshipPicker}
  />
);

export const Route = createFileRoute('/entities/$entityId/')({
  validateSearch: z.object({ relationshipPicker: z.string().optional() }),
  component: EntityPreviewRouteComponent,
});
