import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { BlueprintDetailPage } from '../../../../features/blueprints/BlueprintDetailPage';
import { blueprintDetailTabNames } from '../../../../features/blueprints/constants';

const BlueprintDetailRoute = () => (
  <BlueprintDetailPage
    blueprintId={Route.useParams().blueprintId}
    initialTab={Route.useSearch().tab}
  />
);

export const Route = createFileRoute('/manage/blueprints/$blueprintId/')({
  validateSearch: z.object({
    tab: z.enum(blueprintDetailTabNames).optional().catch(undefined),
  }),
  component: BlueprintDetailRoute,
});
