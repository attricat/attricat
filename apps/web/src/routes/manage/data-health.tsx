import { createFileRoute } from '@tanstack/react-router';
import { DataHealthPage } from '../../features/data-health/DataHealthPage';
import {
  dataHealthSearchSchema,
  type DataHealthSearch,
} from '../../features/data-health/schemas';
import { RequireCapability } from '../../features/auth/RequireCapability';

const parseDataHealthSearch = (
  input: Record<string, unknown>,
): DataHealthSearch => dataHealthSearchSchema.parse(input);

const DataHealthRoute = () => (
  <RequireCapability capability="data_health_read">
    <DataHealthPage search={Route.useSearch()} />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/data-health')({
  validateSearch: parseDataHealthSearch,
  component: DataHealthRoute,
});
