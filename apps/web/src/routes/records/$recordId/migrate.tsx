import { createFileRoute } from '@tanstack/react-router';
import { MigrateRecordPage } from '../../../features/records/MigrateRecordPage';

const MigrateRecordRouteComponent = () => (
  <MigrateRecordPage recordId={Route.useParams().recordId} />
);

export const Route = createFileRoute('/records/$recordId/migrate')({
  component: MigrateRecordRouteComponent,
});
