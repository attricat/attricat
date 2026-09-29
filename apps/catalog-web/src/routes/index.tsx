import { createFileRoute } from '@tanstack/react-router';
import { SavedExplorer } from '../features/saved-views/SavedExplorer';
import { parseExplorerSearch } from '../features/explorer/search';
import { WorkspaceOnboardingGate } from '../features/onboarding/WorkspaceOnboardingGate';

const IndexRouteComponent = () => {
  const search = Route.useSearch();
  return (
    <WorkspaceOnboardingGate
      disabled={Boolean(search.savedView ?? search.viewState)}
    >
      <SavedExplorer search={search} />
    </WorkspaceOnboardingGate>
  );
};

export const Route = createFileRoute('/')({
  validateSearch: parseExplorerSearch,
  component: IndexRouteComponent,
});
