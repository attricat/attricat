import { createFileRoute } from '@tanstack/react-router';
import { DataHealthPage } from '../features/data-health/DataHealthPage';
import {
  dataHealthSearchSchema,
  type DataHealthSearch,
} from '../features/data-health/schemas';

const parseDataHealthSearch = (
  input: Record<string, unknown>,
): DataHealthSearch => dataHealthSearchSchema.parse(input);

const DataHealthRoute = () => <DataHealthPage search={Route.useSearch()} />;

export const Route = createFileRoute('/data-health')({
  validateSearch: parseDataHealthSearch,
  component: DataHealthRoute,
});
