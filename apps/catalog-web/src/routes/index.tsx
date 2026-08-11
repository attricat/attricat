import { createFileRoute } from '@tanstack/react-router';
import { Explorer } from '../features/explorer/Explorer';
import { parseExplorerSearch } from '../features/explorer/search';

const IndexRouteComponent = () => <Explorer search={Route.useSearch()} />;

export const Route = createFileRoute('/')({
  validateSearch: parseExplorerSearch,
  component: IndexRouteComponent,
});
