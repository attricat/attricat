import { useNavigate } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { getCoreRowModel, useLegacyTable } from '@tanstack/react-table/legacy';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Paper } from '@mui/material';
import { lazy, Suspense, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { BlueprintWithAttributes, RecordItem } from '../records/api';
import { DeleteRecordDialog } from '../records/components/DeleteRecordDialog';
import { DuplicateRecordDialog } from '../records/components/DuplicateRecordDialog';
import { getExtensionRuntime } from '../extensions/api';
import { extensionRuntimeRefetchInterval } from '../extensions/constants';
import { extensionQueryKeys } from '../extensions/queryKeys';
import {
  estimatedResultRowHeight,
  maximumExplorerCellFrames,
  loadMoreRowKey,
  resultRowOverscan,
} from './constants';
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
} from './explorerTableColumns';
import type { AttributeFilterDraft } from './attributeFilterValues';
import type { ExplorerSort } from './search';
import { SearchInfoDialog } from './SearchInfoDialog';
import { SendSelectedToAgentDialog } from './SendSelectedToAgentDialog';
import { useRecordPublicationActions } from './useRecordPublicationActions';
import { ApiErrorAlert } from '../../components/CheckViolationsAlert';
import { useExplorerColumnPreferences } from './useExplorerColumnPreferences';
import type { ExplorerSelection } from './useExplorerSelection';
import { VirtualizedExplorerTable } from './VirtualizedExplorerTable';
import { useTimeZone } from '../../time/useInstantFormat';
import { lexiconText } from '../lexicon/lexicon';
import { EmptyState } from '../../components/EmptyState';
import { RecordIcon } from '../../components/systemIcons';

// The column dialog carries drag-and-drop; load it when first opened so it
// stays out of the Explorer's startup bundle.
const ExplorerColumnPreferencesDialog = lazy(() =>
  import('./ExplorerColumnPreferencesDialog').then(
    ({ ExplorerColumnPreferencesDialog }) => ({
      default: ExplorerColumnPreferencesDialog,
    }),
  ),
);

type Props = {
  blueprint: BlueprintWithAttributes;
  hasNextPage: boolean;
  isFetching: boolean;
  isFetchingNextPage: boolean;
  items: RecordItem[];
  onLoadMore: () => void;
  onSortChange: (field: string) => void;
  onFilterCell?: (draft: AttributeFilterDraft) => void;
  onSaveSelectionAsSearch: (records: RecordItem[]) => void;
  /** Opens a result in the record panel instead of navigating to its page. */
  onOpenPanel?: OpenRecordPanel;
  /** The result open in the record panel. */
  panelRecordId?: string;
  /** The selected context followed by its ancestors. */
  contextCodes: readonly string[];
  publicationContextCode: string;
  publicationContextId: string | undefined;
  publicationSortAvailable: boolean;
  canPublish: boolean;
  canDelete: boolean;
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
  onSaveSelectionAsSearch,
  onOpenPanel,
  panelRecordId,
  contextCodes,
  publicationContextCode,
  publicationContextId,
  publicationSortAvailable,
  canPublish,
  canDelete,
  relationshipSortAvailable = true,
  selection,
  showExplorerActions = false,
  sort,
  totalCount,
  totalCountCapped,
}: Props) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
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
    estimateSize: () => estimatedResultRowHeight,
    getItemKey: (index) => items[index]?.id ?? loadMoreRowKey,
    getScrollElement: () => tableContainerRef.current,
    measureElement: (element) => element?.getBoundingClientRect().height,
    overscan: resultRowOverscan,
  });
  const virtualRows = rowVirtualizer.getVirtualItems();
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
  const { error, publicationsByRecordId, publish, readiness, unpublish } =
    useRecordPublicationActions(
      [...publicationItems.values()],
      publicationContextId,
      canPublish ? activeActionRecord?.id : undefined,
    );
  const tableColumns = buildExplorerTableColumns(
    blueprint,
    relationshipSortAvailable,
  );
  const columnIds = configurableColumnIds(tableColumns);
  const columnPreferences = useExplorerColumnPreferences(
    blueprint.blueprint.id,
    columnIds,
  );
  const timeZone = useTimeZone();
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime(),
    queryFn: () => getExtensionRuntime(),
    refetchInterval: extensionRuntimeRefetchInterval,
    retry: false,
  });
  // `flexRender` is only called for virtual rows; see
  // `maximumExplorerCellFrames` for why frames are bounded per render.
  let cellFrames = 0;
  const takeCellFrame = () => cellFrames++ < maximumExplorerCellFrames;
  const columnDefinitions = buildExplorerColumnDefinitions({
    blueprint,
    tableColumns,
    contextCodes,
    publicationContextCode,
    publicationSortAvailable,
    publicationsByRecordId,
    runtime: runtime.data,
    sort,
    onSortChange,
    onFilterCell,
    onOpenActions: (recordId, position) =>
      setActionMenu({ recordId, position }),
    onOpenPanel,
    takeCellFrame,
    t,
    timeZone,
  });
  const columns = arrangeExplorerColumns(
    columnDefinitions,
    columnPreferences.preferences,
    selection.selectionMode,
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
        panelRecordId={panelRecordId}
      />
      {items.length === 0 && (
        <EmptyState icon={RecordIcon} title={t('explorer.noMatchingRecords')} />
      )}
      {actionMenu && activeActionRecord && (
        <RecordActionsMenu
          blueprintId={blueprint.blueprint.id}
          canPublish={canPublish}
          canDelete={canDelete}
          record={activeActionRecord}
          onClose={() => setActionMenu(null)}
          onDelete={() => setDeleteRecordId(activeActionRecord.id)}
          onSearchInfo={setSearchInfoRecord}
          position={actionMenu.position}
          publication={publicationsByRecordId.get(activeActionRecord.id)}
          publicationContextId={publicationContextId}
          publish={() => publish.mutate(activeActionRecord.id)}
          publishing={publish.isPending}
          readiness={readiness}
          duplicate={() => setDuplicateRecordId(activeActionRecord.id)}
          unpublish={() => unpublish.mutate(activeActionRecord.id)}
          unpublishing={unpublish.isPending}
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
