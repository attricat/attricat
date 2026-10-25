import { createFileRoute } from '@tanstack/react-router';
import { DataHealthPage } from '../../features/data-health/DataHealthPage';
import {
  dataHealthSearchSchema,
  type DataHealthSearch,
} from '../../features/data-health/schemas';
import { RequireCapability } from '../../features/auth/RequireCapability';
import { capabilities } from '../../features/auth/capabilities';

const parseDataHealthSearch = (
  input: Record<string, unknown>,
): DataHealthSearch => dataHealthSearchSchema.parse(input);

const DataHealthRoute = () => (
  <RequireCapability capability={capabilities.dataHealthRead}>
    <DataHealthPage search={Route.useSearch()} />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/data-health')({
  validateSearch: parseDataHealthSearch,
  component: DataHealthRoute,
});
