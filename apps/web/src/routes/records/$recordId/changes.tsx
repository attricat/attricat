import { createFileRoute } from '@tanstack/react-router';
import { RecordChangesPage } from '../../../features/records/RecordChangesPage';

const RecordChangesRouteComponent = () => (
  <RecordChangesPage recordId={Route.useParams().recordId} />
);

export const Route = createFileRoute('/records/$recordId/changes')({
  component: RecordChangesRouteComponent,
});
