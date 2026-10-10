import { createFileRoute } from '@tanstack/react-router';
import { SavedExplorer } from '../features/saved-views/SavedExplorer';
import { parseExplorerRouteSearch } from '../features/explorer/search';
import { WorkspaceOnboardingGate } from '../features/onboarding/WorkspaceOnboardingGate';

const IndexRouteComponent = () => {
  const { record, ...search } = Route.useSearch();
  return (
    <WorkspaceOnboardingGate
      disabled={Boolean(search.savedView ?? search.viewState)}
    >
      <SavedExplorer panelRecordId={record} search={search} />
    </WorkspaceOnboardingGate>
  );
};

export const Route = createFileRoute('/')({
  validateSearch: parseExplorerRouteSearch,
  component: IndexRouteComponent,
});
