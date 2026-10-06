import { useQuery } from '@tanstack/react-query';
import { getCoreRowModel, useLegacyTable } from '@tanstack/react-table/legacy';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Paper, Typography } from '@mui/material';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { BlueprintWithAttributes, EntityItem } from '../entities/api';
import { DeleteEntityDialog } from '../entities/components/DeleteEntityDialog';
import { getExtensionRuntime } from '../extensions/api';
import { extensionRuntimeRefetchInterval } from '../extensions/constants';
import { extensionQueryKeys } from '../extensions/queryKeys';
import {
  estimatedResultRowHeight,
  maximumExplorerCellFrames,
  loadMoreRowKey,
  resultRowOverscan,
} from './constants';
import { EntityActionsMenu } from './EntityActionsMenu';
import { ExplorerColumnPreferencesDialog } from './ExplorerColumnPreferencesDialog';
import { ExplorerExtensionActions } from './ExplorerExtensionActions';
import {
  arrangeExplorerColumns,
  buildExplorerColumnDefinitions,
} from './ExplorerResultColumns';
import { ExplorerResultsToolbar } from './ExplorerResultsToolbar';
import type { ActionMenuPosition } from './ExplorerTableCells';
import {
  buildExplorerTableColumns,
  configurableColumnIds,
  explorerColumnLabel,
} from './explorerTableColumns';
import type { AttributeFilterDraft } from './attributeFilterValues';
import type { ExplorerSort } from './search';
import { SearchInfoDialog } from './SearchInfoDialog';
import { SendSelectedToAgentDialog } from './SendSelectedToAgentDialog';
import { useEntityPublicationActions } from './useEntityPublicationActions';
import { ApiErrorAlert } from '../../components/CheckViolationsAlert';
import { useExplorerColumnPreferences } from './useExplorerColumnPreferences';
import type { ExplorerSelection } from './useExplorerSelection';
import { VirtualizedExplorerTable } from './VirtualizedExplorerTable';
import { useTimeZone } from '../../time/useInstantFormat';
import { lexiconText } from '../lexicon/lexicon';

type Props = {
  blueprint: BlueprintWithAttributes;
  hasNextPage: boolean;
  isFetching: boolean;
  isFetchingNextPage: boolean;
  items: EntityItem[];
  onLoadMore: () => void;
  onSortChange: (field: string) => void;
  onFilterCell?: (draft: AttributeFilterDraft) => void;
  onSaveSelectionAsSearch: (entities: EntityItem[]) => void;
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
  const [deleteEntityId, setDeleteEntityId] = useState<string | null>(null);
  const [agentSelection, setAgentSelection] = useState<EntityItem[] | null>(
    null,
  );
  const [searchInfoEntity, setSearchInfoEntity] = useState<EntityItem | null>(
    null,
  );
  const [actionMenu, setActionMenu] = useState<{
    entityId: string;
    position: ActionMenuPosition;
  } | null>(null);
  const [columnPreferencesOpen, setColumnPreferencesOpen] = useState(false);
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
  const activeActionEntity = actionMenu
    ? items.find((item) => item.id === actionMenu.entityId)
    : undefined;
  if (activeActionEntity)
    publicationItems.set(activeActionEntity.id, activeActionEntity);
  const {
    duplicate,
    error,
    publicationsByEntityId,
    publish,
    readiness,
    unpublish,
  } = useEntityPublicationActions(
    [...publicationItems.values()],
    publicationContextId,
    canPublish ? activeActionEntity?.id : undefined,
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
    publicationsByEntityId,
    runtime: runtime.data,
    sort,
    onSortChange,
    onFilterCell,
    onOpenActions: (entityId, position) =>
      setActionMenu({ entityId, position }),
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
  }));
  const table = useLegacyTable({
    data: items,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getRowId: (entity) => entity.id,
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
        onRemoveSelected={selection.removeEntity}
        onSaveSelectionAsSearch={() =>
          onSaveSelectionAsSearch([...selection.selectedItems])
        }
        onSendSelection={() => setAgentSelection([...selection.selectedItems])}
        onToggleSelection={selection.toggleSelectionMode}
        onOpenColumnPreferences={() => setColumnPreferencesOpen(true)}
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
      />
      {items.length === 0 && (
        <Typography sx={{ p: 2 }}>
          {t('explorer.noMatchingEntities')}
        </Typography>
      )}
      {actionMenu && activeActionEntity && (
        <EntityActionsMenu
          blueprintId={blueprint.blueprint.id}
          canPublish={canPublish}
          canDelete={canDelete}
          entity={activeActionEntity}
          onClose={() => setActionMenu(null)}
          onDelete={() => setDeleteEntityId(activeActionEntity.id)}
          onSearchInfo={setSearchInfoEntity}
          position={actionMenu.position}
          publication={publicationsByEntityId.get(activeActionEntity.id)}
          publicationContextId={publicationContextId}
          publish={() => publish.mutate(activeActionEntity.id)}
          publishing={publish.isPending}
          readiness={readiness}
          duplicate={() => duplicate.mutate(activeActionEntity.id)}
          duplicating={duplicate.isPending}
          unpublish={() => unpublish.mutate(activeActionEntity.id)}
          unpublishing={unpublish.isPending}
        />
      )}
      {deleteEntityId && (
        <DeleteEntityDialog
          entityId={deleteEntityId}
          onClose={() => setDeleteEntityId(null)}
          onDeleted={() => {
            selection.removeEntity(deleteEntityId);
            setDeleteEntityId(null);
          }}
        />
      )}
      {agentSelection && (
        <SendSelectedToAgentDialog
          blueprintName={lexiconText(blueprint.blueprint.name)}
          entities={agentSelection}
          onClose={() => setAgentSelection(null)}
          onSuccess={() => {
            setAgentSelection(null);
            selection.exitSelectionMode();
          }}
        />
      )}
      <ExplorerColumnPreferencesDialog
        columns={preferenceColumns}
        onChange={columnPreferences.update}
        onClear={columnPreferences.clear}
        onClose={() => setColumnPreferencesOpen(false)}
        open={columnPreferencesOpen}
        preferences={columnPreferences.preferences}
      />
      <SearchInfoDialog
        entity={searchInfoEntity}
        onClose={() => setSearchInfoEntity(null)}
      />
    </Paper>
  );
};
