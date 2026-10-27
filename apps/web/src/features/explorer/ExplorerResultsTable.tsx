import { useExtensionRuntime } from '../extensions/useExtensionRuntime';
import { useNavigate } from '@tanstack/react-router';
import { useQueryClient } from '@tanstack/react-query';
import { getCoreRowModel, useLegacyTable } from '@tanstack/react-table/legacy';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Paper } from '@mui/material';
import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import type { BlueprintWithAttributes, RecordItem } from '../records/api';
import { DeleteRecordDialog } from '../records/components/DeleteRecordDialog';
import { DuplicateRecordDialog } from '../records/components/DuplicateRecordDialog';
import {
  estimatedResultRowHeight,
  maximumExplorerCellFrames,
  loadMoreRowKey,
  resultRowOverscan,
} from './constants';
import { ExplorerSortContext } from './explorerSortContext';
import { prefetchRecordPanel } from './prefetchRecordPanel';
import { RecordActionsMenu } from './RecordActionsMenu';
import { ExplorerExtensionActions } from './ExplorerExtensionActions';
import {
  arrangeExplorerColumns,
  buildExplorerColumnDefinitions,
} from './ExplorerResultColumns';
import { ExplorerResultsToolbar } from './ExplorerResultsToolbar';
import type { ActionMenuPosition, OpenRecordPanel } from './ExplorerTableCells';
import {
  buildExplorerTableColumns,
  configurableColumnIds,
  explorerColumnLabel,
  explorerColumnValueType,
  usesExtensionRenderer,
} from './explorerTableColumns';
import type { AttributeFilterDraft } from './attributeFilterValues';
import type { ExplorerSort } from './search';
import { SearchInfoDialog } from './SearchInfoDialog';
import { SendSelectedToAgentDialog } from './SendSelectedToAgentDialog';
import { useRecordPublicationActions } from './useRecordPublicationActions';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { useExplorerColumnPreferences } from './useExplorerColumnPreferences';
import type { ExplorerSelection } from './useExplorerSelection';
import { VirtualizedExplorerTable } from './VirtualizedExplorerTable';
import { useTimeZone } from '../../time/useInstantFormat';
import { lexiconText } from '../lexicon/lexicon';
import { EmptyState } from '../../components/EmptyState';
import { RecordIcon } from '../../components/systemIcons';
import { useCan, useCanOn } from '../auth/useCan';
import { useLatestCallback } from '../../components/useLatestCallback';
import { capabilities, accessTargetKinds } from '../auth/capabilities';

// The column dialog carries drag-and-drop; load it when first opened so it
// stays out of the Explorer's startup bundle.
const ExplorerColumnPreferencesDialog = lazy(() =>
  import('./ExplorerColumnPreferencesDialog').then(
    ({ ExplorerColumnPreferencesDialog }) => ({
      default: ExplorerColumnPreferencesDialog,
    }),
  ),
);

// Rows of one blueprint's table have similar heights. A table mounted again,
// such as for a new search, starts from the last measured rows so it renders
// only the rows that fit instead of those the generic estimate allows.
const measuredRowHeights = new Map<string, number>();
const rememberRowHeight = (blueprintId: string, height: number) => {
  const previous = measuredRowHeights.get(blueprintId);
  measuredRowHeights.set(
    blueprintId,
    previous === undefined ? height : (previous * 3 + height) / 4,
  );
};

type Props = {
  blueprint: BlueprintWithAttributes;
  hasNextPage: boolean;
  isFetching: boolean;
  isFetchingNextPage: boolean;
  items: RecordItem[];
  onLoadMore: () => void;
  onSortChange: (field: string) => void;
  onFilterCell?: (draft: AttributeFilterDraft) => void;
  /** Adds a clicked system tag to the search's tag filter. */
  onFilterSystemTag?: (tag: string) => void;
  onSaveSelectionAsSearch: (records: RecordItem[]) => void;
  /** Opens a result in the record panel instead of navigating to its page. */
  onOpenPanel?: OpenRecordPanel;
  /** The selected context followed by its ancestors. */
  contextCodes: readonly string[];
  /** Size the table to its rows instead of filling the viewport. */
  fitRows?: boolean;
  /** Changes when the rows belong to another search; the table then returns to the top. */
  resultsKey?: string;
  publicationContextCode: string;
  publicationContextId: string | undefined;
  publicationSortAvailable: boolean;
  relationshipSortAvailable?: boolean;
  selection: ExplorerSelection;
  showExplorerActions?: boolean;
  sort?: ExplorerSort;
  totalCount: number | null;
  totalCountCapped: boolean;
};

export const ExplorerResultsTable = ({
  blueprint,
  hasNextPage,
  isFetching,
  isFetchingNextPage,
  items,
  onLoadMore,
  onSortChange,
  onFilterCell,
  onFilterSystemTag,
  onSaveSelectionAsSearch,
  onOpenPanel,
  contextCodes,
  fitRows,
  publicationContextCode,
  publicationContextId,
  publicationSortAvailable,
  relationshipSortAvailable = true,
  resultsKey,
  selection,
  showExplorerActions = false,
  sort,
  totalCount,
  totalCountCapped,
}: Props) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [deleteRecordId, setDeleteRecordId] = useState<string | null>(null);
  const [duplicateRecordId, setDuplicateRecordId] = useState<string | null>(
    null,
  );
  const [agentSelection, setAgentSelection] = useState<RecordItem[] | null>(
    null,
  );
  const [searchInfoRecord, setSearchInfoRecord] = useState<RecordItem | null>(
    null,
  );
  const [actionMenu, setActionMenu] = useState<{
    recordId: string;
    position: ActionMenuPosition;
  } | null>(null);
  const [columnPreferencesOpen, setColumnPreferencesOpen] = useState(false);
  // Stays mounted after the first open so closing keeps its transition.
  const [columnPreferencesLoaded, setColumnPreferencesLoaded] = useState(false);
  const tableContainerRef = useRef<HTMLDivElement>(null);
  // Virtualize data subscriptions as well as DOM rows. The core row model is
  // one-to-one with items; server-side sorting already determines their order.
  // eslint-disable-next-line react-hooks/incompatible-library
  const rowVirtualizer = useVirtualizer({
    count: items.length + (hasNextPage ? 1 : 0),
    estimateSize: () =>
      measuredRowHeights.get(blueprint.blueprint.id) ??
      estimatedResultRowHeight,
    getItemKey: (index) => items[index]?.id ?? loadMoreRowKey,
    getScrollElement: () => tableContainerRef.current,
    measureElement: (element) => {
      const height = element.getBoundingClientRect().height;
      if (Number(element.getAttribute('data-index')) < items.length)
        rememberRowHeight(blueprint.blueprint.id, height);
      return height;
    },
    overscan: resultRowOverscan,
  });
  const virtualRows = rowVirtualizer.getVirtualItems();
  useEffect(() => {
    if (tableContainerRef.current) tableContainerRef.current.scrollTop = 0;
  }, [resultsKey]);
  const publicationItems = new Map(
    virtualRows.flatMap(({ index }) =>
      items[index] ? [[items[index].id, items[index]] as const] : [],
    ),
  );
  const activeActionRecord = actionMenu
    ? items.find((item) => item.id === actionMenu.recordId)
    : undefined;
  if (activeActionRecord)
    publicationItems.set(activeActionRecord.id, activeActionRecord);
  const canPublish = useCanOn(
    capabilities.recordsPublish,
    accessTargetKinds.record,
    activeActionRecord?.id,
  );
  const canDelete = useCanOn(
    capabilities.recordsDelete,
    accessTargetKinds.record,
    activeActionRecord?.id,
  );
  const canDuplicate = useCanOn(
    capabilities.recordsWrite,
    accessTargetKinds.record,
    activeActionRecord?.id,
  );
  const {
    error,
    publicationsByRecordId,
    publish,
    publishing,
    readiness,
    unpublish,
    unpublishing,
  } = useRecordPublicationActions(
    [...publicationItems.values()],
    publicationContextId,
    canPublish ? activeActionRecord?.id : undefined,
  );
  const tableColumns = useMemo(
    () => buildExplorerTableColumns(blueprint, relationshipSortAvailable),
    [blueprint, relationshipSortAvailable],
  );
  const dataQualityAvailable = useCan(capabilities.rulesRead);
  const columnIds = configurableColumnIds(tableColumns, dataQualityAvailable);
  const columnPreferences = useExplorerColumnPreferences(
    blueprint.blueprint.id,
    columnIds,
  );
  const timeZone = useTimeZone();
  const runtime = useExtensionRuntime();
  // Rows render only when their own data changes, so the column definitions
  // and every callback they close over keep their identity across renders.
  const sortChange = useLatestCallback(onSortChange);
  const filterCell = useLatestCallback((draft: AttributeFilterDraft) =>
    onFilterCell?.(draft),
  );
  const openPanel = useLatestCallback<Parameters<OpenRecordPanel>, void>(
    (recordId, opener) => onOpenPanel?.(recordId, opener),
  );
  const prefetchPanel = useLatestCallback((record: RecordItem) =>
    prefetchRecordPanel(queryClient, record, blueprint, publicationContextId),
  );
  const openActions = useCallback(
    (recordId: string, position: ActionMenuPosition) =>
      setActionMenu({ recordId, position }),
    [],
  );
  const hasPanel = Boolean(onOpenPanel);
  const hasCellFilter = Boolean(onFilterCell);
  const filterSystemTag = useLatestCallback((tag: string) =>
    onFilterSystemTag?.(tag),
  );
  const hasTagFilter = Boolean(onFilterSystemTag);
  // Extension cell frames are allocated while visible rows render; see
  // `maximumExplorerCellFrames`. Such tables re-render every row each time
  // so the allocation stays bounded.
  const extensionCells = tableColumns.some(usesExtensionRenderer);
  const frameAllocation = extensionCells ? {} : undefined;
  const takeCellFrame = useMemo(() => {
    let cellFrames = 0;
    return () => cellFrames++ < maximumExplorerCellFrames;
    // A new allocation is the only input.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [frameAllocation]);
  const columnDefinitions = useMemo(
    () =>
      buildExplorerColumnDefinitions({
        blueprint,
        tableColumns,
        contextCodes,
        publicationContextCode,
        publicationContextId,
        publicationSortAvailable,
        dataQualityAvailable,
        runtime: runtime.data,
        onSortChange: sortChange,
        onFilterCell: hasCellFilter ? filterCell : undefined,
        onFilterSystemTag: hasTagFilter ? filterSystemTag : undefined,
        onOpenActions: openActions,
        onOpenPanel: hasPanel ? openPanel : undefined,
        onPrefetchPanel: hasPanel ? prefetchPanel : undefined,
        takeCellFrame,
        t,
        timeZone,
      }),
    [
      blueprint,
      contextCodes,
      dataQualityAvailable,
      filterCell,
      filterSystemTag,
      hasCellFilter,
      hasTagFilter,
      hasPanel,
      openActions,
      openPanel,
      prefetchPanel,
      publicationContextCode,
      publicationContextId,
      publicationSortAvailable,
      runtime.data,
      sortChange,
      t,
      tableColumns,
      takeCellFrame,
      timeZone,
    ],
  );
  const columns = useMemo(
    () =>
      arrangeExplorerColumns(
        columnDefinitions,
        columnPreferences.preferences,
        selection.selectionMode,
      ),
    [columnDefinitions, columnPreferences.preferences, selection.selectionMode],
  );
  const preferenceColumns = columnIds.map((id) => ({
    id,
    label: explorerColumnLabel(t, id, tableColumns, publicationContextCode),
    valueType: explorerColumnValueType(blueprint, id),
  }));
  const table = useLegacyTable({
    data: items,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getRowId: (record) => record.id,
  });
  const paddingTop = virtualRows[0]?.start ?? 0;
  const paddingBottom =
    rowVirtualizer.getTotalSize() - (virtualRows.at(-1)?.end ?? 0);

  return (
    <Paper component="section">
      {error && <ApiErrorAlert error={error} sx={{ m: 2 }} />}
      <ExplorerResultsToolbar
        blueprintName={blueprint.blueprint.name}
        itemCount={items.length}
        totalCount={totalCount}
        totalCountCapped={totalCountCapped}
        selectionMode={selection.selectionMode}
        selectedItems={selection.selectedItems}
        onClearSelection={selection.clearSelection}
        onRemoveSelected={selection.removeRecord}
        onSaveSelectionAsSearch={() =>
          onSaveSelectionAsSearch([...selection.selectedItems])
        }
        onSendSelection={() => setAgentSelection([...selection.selectedItems])}
        onToggleSelection={selection.toggleSelectionMode}
        onOpenColumnPreferences={() => {
          setColumnPreferencesLoaded(true);
          setColumnPreferencesOpen(true);
        }}
      />
      {showExplorerActions && (
        <ExplorerExtensionActions
          blueprint={blueprint.blueprint}
          contextId={publicationContextId}
          selectedItems={
            selection.selectionMode ? selection.selectedItems : null
          }
        />
      )}
      <ExplorerSortContext value={sort}>
        <VirtualizedExplorerTable
          table={table}
          columnsLength={columns.length}
          virtualRows={virtualRows}
          paddingTop={paddingTop}
          paddingBottom={paddingBottom}
          measureElement={rowVirtualizer.measureElement}
          tableContainerRef={tableContainerRef}
          selection={selection}
          items={items}
          isFetching={isFetching}
          isFetchingNextPage={isFetchingNextPage}
          onLoadMore={onLoadMore}
          highlightPanelRecord={hasPanel}
          renderToken={frameAllocation}
          fitRows={fitRows}
        />
      </ExplorerSortContext>
      {items.length === 0 && (
        <EmptyState icon={RecordIcon} title={t('explorer.noMatchingRecords')} />
      )}
      {actionMenu && activeActionRecord && (
        <RecordActionsMenu
          blueprintId={blueprint.blueprint.id}
          canPublish={canPublish}
          canDelete={canDelete}
          canDuplicate={canDuplicate}
          record={activeActionRecord}
          onClose={() => setActionMenu(null)}
          onDelete={() => setDeleteRecordId(activeActionRecord.id)}
          onSearchInfo={setSearchInfoRecord}
          position={actionMenu.position}
          publication={publicationsByRecordId.get(activeActionRecord.id)}
          publicationContextId={publicationContextId}
          publish={() => publish(activeActionRecord.id)}
          publishing={publishing}
          readiness={readiness}
          duplicate={() => setDuplicateRecordId(activeActionRecord.id)}
          unpublish={() => unpublish(activeActionRecord.id)}
          unpublishing={unpublishing}
        />
      )}
      {duplicateRecordId && (
        <DuplicateRecordDialog
          recordId={duplicateRecordId}
          onClose={() => setDuplicateRecordId(null)}
          onDuplicated={(copy) => {
            setDuplicateRecordId(null);
            void navigate({
              params: { recordId: copy.id },
              to: '/records/$recordId',
            });
          }}
        />
      )}
      {deleteRecordId && (
        <DeleteRecordDialog
          recordId={deleteRecordId}
          onClose={() => setDeleteRecordId(null)}
          onDeleted={() => {
            selection.removeRecord(deleteRecordId);
            setDeleteRecordId(null);
          }}
        />
      )}
      {agentSelection && (
        <SendSelectedToAgentDialog
          blueprintName={lexiconText(blueprint.blueprint.name)}
          records={agentSelection}
          onClose={() => setAgentSelection(null)}
          onSuccess={() => {
            setAgentSelection(null);
            selection.exitSelectionMode();
          }}
        />
      )}
      {columnPreferencesLoaded && (
        <Suspense fallback={null}>
          <ExplorerColumnPreferencesDialog
            columns={preferenceColumns}
            onChange={columnPreferences.update}
            onClear={columnPreferences.clear}
            onClose={() => setColumnPreferencesOpen(false)}
            open={columnPreferencesOpen}
            preferences={columnPreferences.preferences}
          />
        </Suspense>
      )}
      <SearchInfoDialog
        record={searchInfoRecord}
        onClose={() => setSearchInfoRecord(null)}
      />
    </Paper>
  );
};
