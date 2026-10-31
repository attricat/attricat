import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { RecordPreviewPage } from '../../../features/records/RecordPreviewPage';

const RecordPreviewRouteComponent = () => {
  const search = Route.useSearch();
  return (
    <RecordPreviewPage
      context={search.context}
      recordId={Route.useParams().recordId}
      relationshipPickerToken={search.relationshipPicker}
      tab={search.tab}
    />
  );
};

export const Route = createFileRoute('/records/$recordId/')({
  validateSearch: z.object({
    /** The context shown; the default context when absent. */
    context: z.string().optional(),
    relationshipPicker: z.string().optional(),
    /** The open tab; an unavailable one shows the record's data. */
    tab: z.string().optional(),
  }),
  component: RecordPreviewRouteComponent,
});
