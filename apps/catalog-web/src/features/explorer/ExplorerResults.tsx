import { Alert, Box } from '@mui/material';
import { ExplorerResultsTable } from './ExplorerResultsTable';
import { HiddenOutdatedNotice } from './HiddenOutdatedNotice';
import type { AttributeFilterDraft } from './attributeFilterValues';
import type { ExplorerSearch } from './search';
import type { ExplorerData } from './useExplorerData';
import { useExplorerSelection } from './useExplorerSelection';

type Props = {
  data: ExplorerData;
  onFilterCell: (draft: AttributeFilterDraft) => void;
  onShowAllVersions: () => void;
  onSortChange: (field: string) => void;
  search: ExplorerSearch;
};

export const ExplorerResults = ({
  data,
  onFilterCell,
  onShowAllVersions,
  onSortChange,
  search,
}: Props) => {
  const {
    contextCode,
    contextId,
    currentBlueprint,
    effectiveVersion,
    publicationSortAvailable,
    relationshipFilters,
    results,
    session,
  } = data;
  const capabilities = session.data?.capabilities;
  const resultPages = results.data?.pages ?? [];
  const firstPage = resultPages[0];
  const resultBlueprint = firstPage?.blueprint;
  const singleVersionScope =
    firstPage?.result_version_scope.kind === 'single'
      ? firstPage.result_version_scope
      : undefined;
  const hiddenOutdatedCount = firstPage?.hidden_outdated_count ?? 0;
  const items = resultPages.flatMap((page) => page.items);
  // The table remounts for every search; selection outlives it.
  const selection = useExplorerSelection(search.blueprint, items);

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
                canReviewMigrations={Boolean(capabilities?.data_health_read)}
                capped={Boolean(firstPage?.hidden_outdated_count_capped)}
                count={hiddenOutdatedCount}
                onShowAllVersions={onShowAllVersions}
              />
            )}
          {resultBlueprint && (
            <ExplorerResultsTable
              blueprint={resultBlueprint}
              key={JSON.stringify([
                search.blueprint,
                effectiveVersion,
                search.allVersions,
                search.query,
                search.sort,
                search.attributeFilters,
                relationshipFilters,
              ])}
              canPublish={capabilities?.entities_publish === true}
              canDelete={capabilities?.entities_delete === true}
              hasNextPage={results.hasNextPage}
              isFetching={results.isFetching}
              isFetchingNextPage={results.isFetchingNextPage}
              items={items}
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
              onSortChange={onSortChange}
              sort={search.sort}
            />
          )}
        </Box>
      )}
    </>
  );
};
