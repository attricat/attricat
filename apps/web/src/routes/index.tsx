import { createFileRoute } from '@tanstack/react-router';
import { SavedExplorer } from '../features/saved-views/SavedExplorer';
import {
  parseExplorerRouteSearch,
  withoutPanelRecord,
} from '../features/explorer/search';
import { PanelRecordProvider } from '../features/explorer/PanelRecordProvider';
import { WorkspaceOnboardingGate } from '../features/onboarding/WorkspaceOnboardingGate';

const IndexRouteComponent = () => {
  // The search keeps its identity while only the panel record changes, so
  // opening a record re-renders the panel and not the results.
  const search = Route.useSearch({
    select: withoutPanelRecord,
    structuralSharing: true,
  });
  const record = Route.useSearch({ select: ({ record }) => record });
  return (
    <WorkspaceOnboardingGate
      disabled={Boolean(search.savedView ?? search.viewState)}
    >
      <PanelRecordProvider recordId={record}>
        <SavedExplorer search={search} />
      </PanelRecordProvider>
    </WorkspaceOnboardingGate>
  );
};

export const Route = createFileRoute('/')({
  validateSearch: parseExplorerRouteSearch,
  component: IndexRouteComponent,
});
