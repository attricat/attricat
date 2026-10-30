import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { RecordPreviewPage } from '../../../features/records/RecordPreviewPage';

const RecordPreviewRouteComponent = () => {
  const search = Route.useSearch();
  return (
    <RecordPreviewPage
      recordId={Route.useParams().recordId}
      relationshipPickerToken={search.relationshipPicker}
      tab={search.tab}
    />
  );
};

export const Route = createFileRoute('/records/$recordId/')({
  validateSearch: z.object({
    relationshipPicker: z.string().optional(),
    /** The open tab; an unavailable one shows the record's data. */
    tab: z.string().optional(),
  }),
  component: RecordPreviewRouteComponent,
});
