import { useNavigate } from '@tanstack/react-router';
import { useVirtualizer } from '@tanstack/react-virtual';
import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import {
  getCoreRowModel,
  legacyCreateColumnHelper,
  type LegacyColumnDef,
  useLegacyTable,
} from '@tanstack/react-table/legacy';
import {
  Alert,
  Dialog,
  DialogContent,
  DialogTitle,
  Paper,
  Typography,
} from '@mui/material';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  duplicateEntity,
  getEntityPublications,
  publishEntity,
  unpublishEntity,
  type BlueprintWithAttributes,
  type EntityItem,
  type EntityPublicationStatus,
} from '../entities/api';
import { DeleteEntityDialog } from '../entities/components/DeleteEntityDialog';
import { getExtensionRuntime } from '../extensions/api';
import { extensionQueryKeys } from '../extensions/queryKeys';
import { extensionRuntimeRefetchInterval } from '../extensions/constants';
import { EntityActionsMenu } from './EntityActionsMenu';
import { ExplorerColumnPreferencesDialog } from './ExplorerColumnPreferencesDialog';
import { ExplorerResultsToolbar } from './ExplorerResultsToolbar';
import {
  buildExplorerColumnDefinitions,
  type ExplorerTableColumn,
} from './ExplorerResultColumns';
import { SendSelectedToAgentDialog } from './SendSelectedToAgentDialog';
import { VirtualizedExplorerTable } from './VirtualizedExplorerTable';
import { useExplorerSelection } from './useExplorerSelection';
import { entityQueryKeys } from '../entities/queryKeys';
import {
  clearExplorerColumnPreferences,
  getExplorerColumnPreferences,
  setExplorerColumnPreferences,
  type ExplorerColumnPreferences,
} from './columnPreferences';

const maximumExplorerCellFrames = 32;

export const ExplorerResultsTable = ({
  blueprint,
  hasNextPage,
  isFetching,
  isFetchingNextPage,
  items,
  onLoadMore,
  onSortChange,
  publicationContextCode,
  publicationContextId,
  publicationSortAvailable,
  canPublish,
  canDelete,
  relationshipSortAvailable = true,
  sort,
  totalCount,
  totalCountCapped,
}: {
  blueprint: BlueprintWithAttributes;
  hasNextPage: boolean;
  isFetching: boolean;
  isFetchingNextPage: boolean;
  items: EntityItem[];
  onLoadMore: () => void;
  onSortChange: (field: string) => void;
  publicationContextCode: string;
  publicationContextId: string | undefined;
  publicationSortAvailable: boolean;
  canPublish: boolean;
  canDelete: boolean;
  relationshipSortAvailable?: boolean;
  sort?: { field: string; direction: 'asc' | 'desc' };
  totalCount: number | null;
  totalCountCapped: boolean;
}) => {
  const { t } = useTranslation();
  const selection = useExplorerSelection(items);
  const [deleteEntityId, setDeleteEntityId] = useState<string | null>(null);
  const [agentSelection, setAgentSelection] = useState<EntityItem[] | null>(
    null,
  );
  const [searchInfoEntity, setSearchInfoEntity] = useState<EntityItem | null>(
    null,
  );
  const [actionMenu, setActionMenu] = useState<{
    entityId: string;
    position: { left: number; top: number };
  } | null>(null);
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const publicationQueries = useQueries({
    queries: items.map((entity) => ({
      queryKey: entityQueryKeys.publication(entity.id),
      queryFn: () => getEntityPublications(entity.id),
      enabled: Boolean(publicationContextId),
    })),
  });
  const publicationsByEntityId = new Map(
    items.map((entity, index) => [
      entity.id,
      publicationQueries[index]?.data?.find(
        (publication) => publication.context_id === publicationContextId,
      ),
    ]),
  );
  const updatePublication = (
    entityId: string,
    publication: EntityPublicationStatus,
  ) =>
    queryClient.setQueryData<EntityPublicationStatus[]>(
      entityQueryKeys.publication(entityId),
      (current) => [
        ...(current ?? []).filter(
          (item) => item.context_id !== publication.context_id,
        ),
        publication,
      ],
    );
  const invalidatePublication = (entityId: string) =>
    queryClient.invalidateQueries({
      queryKey: entityQueryKeys.publication(entityId),
    });
  const publish = useMutation({
    mutationFn: (entityId: string) => {
      if (!publicationContextId)
        throw new Error('Publication context is unavailable');
      return publishEntity(entityId, publicationContextId);
    },
    onSuccess: (publication, entityId) => {
      updatePublication(entityId, publication);
      void invalidatePublication(entityId);
    },
  });
  const unpublish = useMutation({
    mutationFn: (entityId: string) => {
      if (!publicationContextId)
        throw new Error('Publication context is unavailable');
      return unpublishEntity(entityId, publicationContextId);
    },
    onSuccess: (_, entityId) => void invalidatePublication(entityId),
  });
  const duplicate = useMutation({
    mutationFn: (entityId: string) => duplicateEntity(entityId),
    onSuccess: (entity) => {
      void queryClient.invalidateQueries({
        queryKey: entityQueryKeys.searches(),
      });
      void navigate({
        params: { entityId: entity.id },
        to: '/entities/$entityId/edit',
      });
    },
  });
  const columnHelper = legacyCreateColumnHelper<EntityItem>();
  const tableView =
    blueprint.blueprint.views.table?.type === 'table'
      ? blueprint.blueprint.views.table
      : undefined;
  const tableColumns: ExplorerTableColumn[] = tableView?.columns?.length
    ? tableView.columns.map((column) => {
        const configuredSortable =
          blueprint.table_path_attributes.find(
            (attribute) => attribute.code === column.field,
          )?.sortable ?? false;
        const relationshipSortBlocked =
          configuredSortable &&
          column.field.includes('.') &&
          !relationshipSortAvailable;
        return {
          ...column,
          relationshipSortBlocked,
          sortable: configuredSortable && !relationshipSortBlocked,
        };
      })
    : (tableView?.fields ?? []).map((field) => ({
        field,
        relationshipSortBlocked: false,
        sortable: false,
      }));
  const configurableColumnIds = [
    'id',
    'display',
    'publication',
    'schema',
    ...tableColumns.map((column) => column.field),
  ];
  const [columnPreferences, setColumnPreferences] =
    useState<ExplorerColumnPreferences>(() =>
      getExplorerColumnPreferences(
        blueprint.blueprint.id,
        configurableColumnIds,
      ),
    );
  const [columnPreferencesOpen, setColumnPreferencesOpen] = useState(false);
  const updateColumnPreferences = (preferences: ExplorerColumnPreferences) => {
    setColumnPreferences(preferences);
    setExplorerColumnPreferences(blueprint.blueprint.id, preferences);
  };
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime(),
    queryFn: () => getExtensionRuntime(),
    refetchInterval: extensionRuntimeRefetchInterval,
    retry: false,
  });
  // `flexRender` is only called for virtual rows. This bounded allocator keeps
  // a pathological blueprint from turning one Explorer viewport into hundreds
  // of opaque-origin frames.
  let cellFrames = 0;
  const takeCellFrame = () => cellFrames++ < maximumExplorerCellFrames;
  // Render selection outside the legacy table's memoized column definitions so
  // checkbox state follows selection changes even when result rows do not.
  const selectionColumn = columnHelper.display({
    id: 'select',
    header: '',
    cell: () => null,
  }) as LegacyColumnDef<EntityItem, unknown>;
  const columnDefinitions = buildExplorerColumnDefinitions({
    blueprint,
    tableColumns,
    publicationContextCode,
    publicationSortAvailable,
    publicationsByEntityId,
    runtime: runtime.data,
    sort,
    onSortChange,
    onOpenActions: (entityId, position) =>
      setActionMenu({ entityId, position }),
    takeCellFrame,
    t,
  });
  const actionColumn = columnDefinitions.pop();
  const columns = [
    ...(selection.selectionMode ? [selectionColumn] : []),
    ...columnPreferences.order.flatMap((id) => {
      const column = columnDefinitions.find(
        (definition) => definition.id === id,
      );
      return column && !columnPreferences.hidden.includes(id) ? [column] : [];
    }),
    ...(actionColumn ? [actionColumn] : []),
  ];
  const preferenceColumns = configurableColumnIds.map((id) => ({
    id,
    label:
      id === 'id'
        ? t('explorer.id')
        : id === 'display'
          ? t('explorer.display')
          : id === 'publication'
            ? t('explorer.publicationForContext', {
                context: publicationContextCode,
              })
            : id === 'schema'
              ? t('explorer.schema')
              : (tableColumns.find((column) => column.field === id)?.label ??
                id.replaceAll('_', ' ')),
  }));
  const table = useLegacyTable({
    data: items,
    columns,
    getCoreRowModel: getCoreRowModel(),
  });
  const rows = table.getRowModel().rows;
  const tableContainerRef = useRef<HTMLDivElement>(null);
  // TanStack Virtual owns imperative scroll measurements and is intentionally
  // excluded from React Compiler memoization.
  // eslint-disable-next-line react-hooks/incompatible-library
  const rowVirtualizer = useVirtualizer({
    count: rows.length + (hasNextPage ? 1 : 0),
    estimateSize: () => 53,
    getScrollElement: () => tableContainerRef.current,
    measureElement: (element) => element?.getBoundingClientRect().height,
    overscan: 10,
  });
  const virtualRows = rowVirtualizer.getVirtualItems();
  const paddingTop = virtualRows[0]?.start ?? 0;
  const paddingBottom =
    rowVirtualizer.getTotalSize() - (virtualRows.at(-1)?.end ?? 0);
  const activeActionEntity = actionMenu
    ? items.find((item) => item.id === actionMenu.entityId)
    : undefined;

  return (
    <Paper component="section">
      {(publish.isError || unpublish.isError || duplicate.isError) && (
        <Alert severity="error" sx={{ m: 2 }}>
          {publish.error?.message ??
            unpublish.error?.message ??
            duplicate.error?.message}
        </Alert>
      )}
      <ExplorerResultsToolbar
        itemCount={items.length}
        totalCount={totalCount}
        totalCountCapped={totalCountCapped}
        selectionMode={selection.selectionMode}
        selectedCount={selection.selectedItems.length}
        onClearSelection={selection.clearSelection}
        onSendSelection={() => setAgentSelection([...selection.selectedItems])}
        onToggleSelection={selection.toggleSelectionMode}
        onOpenColumnPreferences={() => setColumnPreferencesOpen(true)}
      />
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
          blueprintName={blueprint.blueprint.name}
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
        onChange={updateColumnPreferences}
        onClear={() => {
          clearExplorerColumnPreferences(blueprint.blueprint.id);
          setColumnPreferences({
            hidden: [],
            order: configurableColumnIds,
          });
        }}
        onClose={() => setColumnPreferencesOpen(false)}
        open={columnPreferencesOpen}
        preferences={columnPreferences}
      />
      <Dialog
        onClose={() => setSearchInfoEntity(null)}
        open={Boolean(searchInfoEntity)}
      >
        <DialogTitle>{t('explorer.searchInfo')}</DialogTitle>
        <DialogContent>
          <Typography>
            {searchInfoEntity?.match_explanations
              .map((explanation) =>
                explanation.traversal_depth
                  ? t('explorer.matchViaRelationship', {
                      count: explanation.traversal_depth,
                      term: explanation.term,
                    })
                  : explanation.matching_attribute_code
                    ? t('explorer.matchInAttribute', {
                        attribute: explanation.matching_attribute_code,
                        term: explanation.term,
                      })
                    : explanation.term,
              )
              .join('; ') || t('explorer.noSearchDetails')}
          </Typography>
        </DialogContent>
      </Dialog>
    </Paper>
  );
};
