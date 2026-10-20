import { Alert, Box } from '@mui/material';
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

type Props = {
  data: ExplorerData;
  onFilterCell: (draft: AttributeFilterDraft) => void;
  onOpenPanel?: OpenRecordPanel;
  onShowAllVersions: () => void;
  onSortChange: (field: string) => void;
  search: ExplorerSearch;
};

export const ExplorerResults = ({
  data,
  onFilterCell,
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
  const canReadDataHealth = useCan('data_health_read');
  const resultPages = results.data?.pages ?? [];
  const firstPage = resultPages[0];
  const resultBlueprint = firstPage?.blueprint;
  const singleVersionScope =
    firstPage?.result_version_scope.kind === 'single'
      ? firstPage.result_version_scope
      : undefined;
  const hiddenOutdatedCount = firstPage?.hidden_outdated_count ?? 0;
  const items = useMemo(
    () => (results.data?.pages ?? []).flatMap((page) => page.items),
    [results.data?.pages],
  );
  // The table remounts for every search; selection outlives it. Until the
  // new search's rows arrive, the previous rows stay in the previous table
  // rather than mounting a new table for them only to replace them.
  const searchKey = JSON.stringify([
    search.blueprint,
    effectiveVersion,
    search.allVersions,
    search.query,
    search.sort,
    search.attributeFilters,
    relationshipFilters,
    relationshipTreeSelections,
  ]);
  const [tableKey, setTableKey] = useState(searchKey);
  if (tableKey !== searchKey && !results.isPlaceholderData)
    setTableKey(searchKey);
  const selection = useExplorerSelection(search.blueprint, items);
  const [savedSearchSelection, setSavedSearchSelection] = useState<
    RecordItem[] | null
  >(null);

  return (
    <>
      {results.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {results.error.message}
        </Alert>
      )}
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
