import { Box, Button } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ApiRequestError } from '../../api/request';
import { isClientErrorStatus } from '../../api/httpStatus';
import { useEffect, useMemo, useState } from 'react';
import { searchRecords } from '../records/api';
import { recordIdsQuery } from './queryLanguage';
import {
  forgetPickerCreatedRecords,
  readPickerCreatedRecords,
} from './relationshipPickerStorage';
import { useLatestCallback } from '../../components/useLatestCallback';
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
import { useRelationshipPickerSession } from './relationshipPickerSession';
import { RelationshipPickerSelectionBar } from './RelationshipPickerSelectionBar';
import { relationshipPickerModes } from '../records/constants';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import {
  resultsShowBlueprintRevision,
  singleResultVersionScope,
} from '../records/schemas';

type Props = {
  data: ExplorerData;
  onFilterCell: (draft: AttributeFilterDraft) => void;
  onFilterSystemTag: (tag: string) => void;
  onOpenPanel?: OpenRecordPanel;
  onShowAllVersions: () => void;
  onClearSort: () => void;
  onSortChange: (field: string) => void;
  search: ExplorerSearch;
};

export const ExplorerResults = ({
  data,
  onFilterCell,
  onFilterSystemTag,
  onOpenPanel,
  onShowAllVersions,
  onClearSort,
  onSortChange,
  search,
}: Props) => {
  const { t } = useTranslation();
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
  // A tab a relationship picker opened only chooses records for it.
  const picker = useRelationshipPickerSession();
  const selection = useExplorerSelection(
    search.blueprint,
    items,
    picker
      ? {
          alwaysSelecting: true,
          storageToken: picker.token,
          ...(picker.mode === relationshipPickerModes.one
            ? { maximum: 1 }
            : {}),
        }
      : {},
  );
  // Records created from this picker tab join its selection when it returns.
  // They are forgotten only once added, so an interrupted lookup retries.
  const addCreated = useLatestCallback(
    (token: string, ids: string[], items: RecordItem[]) => {
      selection.addRecords(items);
      forgetPickerCreatedRecords(token, ids);
    },
  );
  const pickerToken = picker?.token;
  const blueprintCode = search.blueprint;
  useEffect(() => {
    if (!pickerToken || !blueprintCode) return;
    const created = readPickerCreatedRecords(pickerToken);
    if (created.length === 0) return;
    const controller = new AbortController();
    void searchRecords({
      blueprint: blueprintCode,
      query: recordIdsQuery(created),
      pageSize: created.length,
      signal: controller.signal,
    })
      .then((page) => addCreated(pickerToken, created, page.items))
      .catch(() => {
        // An aborted or failed lookup leaves the selection as it was.
      });
    return () => controller.abort();
  }, [addCreated, blueprintCode, pickerToken]);
  const [savedSearchSelection, setSavedSearchSelection] = useState<
    RecordItem[] | null
  >(null);

  return (
    <>
      <ApiErrorAlert
        action={
          // A sort can stop applying when the context or revision changes,
          // and the headers that would change it go with the table.
          search.sort &&
          results.error instanceof ApiRequestError &&
          isClientErrorStatus(results.error.status) && (
            <Button color="inherit" onClick={onClearSort} size="small">
              {t('explorer.clearSort')}
            </Button>
          )
        }
        error={results.error}
        sx={{ mt: 3 }}
      />
      {results.data && (
        <Box sx={{ mt: 3 }}>
          {picker && (
            <RelationshipPickerSelectionBar
              onClear={selection.clearSelection}
              onRemove={selection.removeRecord}
              selectedItems={selection.selectedItems}
              session={picker}
            />
          )}
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
                !picker && resultsShowBlueprintRevision(firstPage)
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
