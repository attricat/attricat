import { createFileRoute } from '@tanstack/react-router';
import { SavedExplorer } from '../features/saved-views/SavedExplorer';
import { parseExplorerSearch } from '../features/explorer/search';

const IndexRouteComponent = () => <SavedExplorer search={Route.useSearch()} />;

export const Route = createFileRoute('/')({
  validateSearch: parseExplorerSearch,
  component: IndexRouteComponent,
});
