import { createFileRoute, retainSearchParams } from '@tanstack/react-router';
import { SavedExplorer } from '../features/saved-views/SavedExplorer';
import {
  parseExplorerRouteSearch,
  relationshipPickerSession,
  withoutPanelRecord,
} from '../features/explorer/search';
import { RelationshipPickerSessionContext } from '../features/explorer/relationshipPickerSession';
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
  const picker = Route.useSearch({
    select: relationshipPickerSession,
    structuralSharing: true,
  });
  return (
    <WorkspaceOnboardingGate
      disabled={Boolean(search.savedView ?? search.viewState ?? picker)}
    >
      <RelationshipPickerSessionContext value={picker}>
        <PanelRecordProvider recordId={record}>
          <SavedExplorer search={search} />
        </PanelRecordProvider>
      </RelationshipPickerSessionContext>
    </WorkspaceOnboardingGate>
  );
};

export const Route = createFileRoute('/')({
  validateSearch: parseExplorerRouteSearch,
  // A relationship picker tab keeps its picker while the search changes, so
  // the selection can still be sent back after filtering or searching.
  search: {
    middlewares: [
      retainSearchParams(['relationshipPicker', 'relationshipPickerMode']),
    ],
  },
  component: IndexRouteComponent,
});
