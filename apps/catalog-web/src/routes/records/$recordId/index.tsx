import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { RecordPreviewPage } from '../../../features/records/RecordPreviewPage';

const RecordPreviewRouteComponent = () => (
  <RecordPreviewPage
    recordId={Route.useParams().recordId}
    relationshipPickerToken={Route.useSearch().relationshipPicker}
  />
);

export const Route = createFileRoute('/records/$recordId/')({
  validateSearch: z.object({ relationshipPicker: z.string().optional() }),
  component: RecordPreviewRouteComponent,
});
