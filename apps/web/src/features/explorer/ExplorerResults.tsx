import { Box } from '@mui/material';
import { useMemo, useState } from 'react';
import { useCan } from '../auth/useCan';
import type { RecordItem } from '../records/api';
import { SaveSelectionAsSearchDialog } from '../saved-views/SaveSelectionAsSearchDialog';
import { ExplorerResultsTable } from './ExplorerResultsTable';
import { HiddenOutdatedNotice } from './HiddenOutdatedNotice';
import type { AttributeFilterDraft } from './attributeFilterValues';
import type { OpenRecordPanel } from './ExplorerTableCells';
import type { ExplorerSearch } from './search';
import type { ExplorerData } from './useExplorerData';
import { useExplorerSelection } from './useExplorerSelection';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { singleResultVersionScope } from '../records/schemas';

type Props = {
  data: ExplorerData;
  onFilterCell: (draft: AttributeFilterDraft) => void;
  onFilterSystemTag: (tag: string) => void;
  onOpenPanel?: OpenRecordPanel;
  onShowAllVersions: () => void;
  onSortChange: (field: string) => void;
  search: ExplorerSearch;
};

export const ExplorerResults = ({
  data,
  onFilterCell,
  onFilterSystemTag,
  onOpenPanel,
  onShowAllVersions,
  onSortChange,
  search,
}: Props) => {
  const {
    contextCode,
    contextCodes,
    contextId,
    currentBlueprint,
    effectiveVersion,
    publicationSortAvailable,
    relationshipFilters,
    relationshipTreeSelections,
    results,
  } = data;
  const canReadDataHealth = useCan(capabilities.dataHealthRead);
  const resultPages = results.data?.pages ?? [];
  const firstPage = resultPages[0];
  const resultBlueprint = firstPage?.blueprint;
  const singleVersionScope = singleResultVersionScope(firstPage);
  const hiddenOutdatedCount = firstPage?.hidden_outdated_count ?? 0;
  const items = useMemo(
    () => (results.data?.pages ?? []).flatMap((page) => page.items),
    [results.data?.pages],
  );
  // The table remounts for the rows of another blueprint revision, whose
  // columns and preferences differ; selection outlives it. Another search of
  // the same revision keeps the table, and any menu open in it, and returns
  // to the top once the new search's rows arrive.
  const tableKey = JSON.stringify([
    resultBlueprint?.blueprint.id,
    resultBlueprint?.blueprint.version,
  ]);
  const searchKey = JSON.stringify([
    search.blueprint,
    effectiveVersion,
    search.allVersions,
    search.query,
    search.sort,
    search.attributeFilters,
    relationshipFilters,
    relationshipTreeSelections,
    search.exportStatus,
    search.systemTags,
    search.dataQuality,
  ]);
  const [shownSearch, setShownSearch] = useState(searchKey);
  if (shownSearch !== searchKey && !results.isPlaceholderData)
    setShownSearch(searchKey);
  const selection = useExplorerSelection(search.blueprint, items);
  const [savedSearchSelection, setSavedSearchSelection] = useState<
    RecordItem[] | null
  >(null);

  return (
    <>
      <ApiErrorAlert error={results.error} sx={{ mt: 3 }} />
      {results.data && (
        <Box sx={{ mt: 3 }}>
          {!search.allVersions &&
            effectiveVersion === currentBlueprint?.version &&
            hiddenOutdatedCount > 0 && (
              <HiddenOutdatedNotice
                canReviewMigrations={canReadDataHealth}
                capped={Boolean(firstPage?.hidden_outdated_count_capped)}
                count={hiddenOutdatedCount}
                onShowAllVersions={onShowAllVersions}
              />
            )}
          {resultBlueprint && (
            <ExplorerResultsTable
              blueprint={resultBlueprint}
              key={tableKey}
              resultsKey={shownSearch}
              hasNextPage={results.hasNextPage}
              isFetching={results.isFetching}
              isFetchingNextPage={results.isFetchingNextPage}
              items={items}
              contextCodes={contextCodes}
              publicationContextCode={contextCode}
              publicationContextId={contextId}
              publicationSortAvailable={publicationSortAvailable}
              totalCount={firstPage?.total_count ?? null}
              totalCountCapped={firstPage?.total_count_capped ?? false}
              onLoadMore={() => void results.fetchNextPage()}
              selection={selection}
              relationshipSortAvailable={
                !search.allVersions || Boolean(singleVersionScope)
              }
              showExplorerActions={
                singleVersionScope?.version ===
                resultBlueprint.blueprint.version
              }
              onFilterCell={onFilterCell}
              onFilterSystemTag={onFilterSystemTag}
              onOpenPanel={onOpenPanel}
              onSaveSelectionAsSearch={setSavedSearchSelection}
              onSortChange={onSortChange}
              sort={search.sort}
            />
          )}
        </Box>
      )}
      {savedSearchSelection && (
        <SaveSelectionAsSearchDialog
          records={savedSearchSelection}
          onClose={() => setSavedSearchSelection(null)}
          onSuccess={() => {
            setSavedSearchSelection(null);
            selection.exitSelectionMode();
          }}
          search={search}
        />
      )}
    </>
  );
};
