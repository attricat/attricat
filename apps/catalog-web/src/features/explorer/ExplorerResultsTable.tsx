import { useNavigate } from '@tanstack/react-router';
import { flexRender } from '@tanstack/react-table';
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
  Box,
  Checkbox,
  Dialog,
  DialogContent,
  DialogTitle,
  LinearProgress,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
  type SxProps,
  type Theme,
} from '@mui/material';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { useEffect, useRef, useState } from 'react';
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
import { displayLabel } from '../entities/entity-display';
import { getExtensionRuntime } from '../extensions/api';
import { extensionQueryKeys } from '../extensions/query-keys';
import { extensionRuntimeRefetchInterval } from '../extensions/constants';
import { EntityActionsMenu } from './EntityActionsMenu';
import { ExplorerColumnPreferencesDialog } from './ExplorerColumnPreferencesDialog';
import { ExplorerResultsToolbar } from './ExplorerResultsToolbar';
import {
  buildExplorerColumnDefinitions,
  type ExplorerTableColumn,
} from './ExplorerResultColumns';
import { maximumAgentSelection } from './agent-selection';
import { SendSelectedToAgentDialog } from './SendSelectedToAgentDialog';
import { entityQueryKeys } from '../entities/query-keys';
import {
  clearExplorerColumnPreferences,
  getExplorerColumnPreferences,
  setExplorerColumnPreferences,
  type ExplorerColumnPreferences,
} from './column-preferences';

const maximumExplorerCellFrames = 32;

const resultColumnCellSx = (columnId: string): SxProps<Theme> =>
  columnId === 'display'
    ? { minWidth: 280 }
    : columnId === 'id'
      ? { textAlign: 'center', width: 48 }
      : {};

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
  canPublish: boolean;
  canDelete: boolean;
  relationshipSortAvailable?: boolean;
  sort?: { field: string; direction: 'asc' | 'desc' };
  totalCount: number | null;
  totalCountCapped: boolean;
}) => {
  const { t } = useTranslation();
  const [selectionMode, setSelectionMode] = useState(false);
  const [deleteEntityId, setDeleteEntityId] = useState<string | null>(null);
  const [selectedEntityIds, setSelectedEntityIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [agentSelection, setAgentSelection] = useState<EntityItem[] | null>(
    null,
  );
  const selectedItems = items.filter((item) => selectedEntityIds.has(item.id));
  const allLoadedSelected =
    items.length > 0 &&
    items
      .slice(0, maximumAgentSelection)
      .every((item) => selectedEntityIds.has(item.id));
  useEffect(() => {
    const loaded = new Set(items.map((item) => item.id));
    setSelectedEntityIds((current) => {
      const next = new Set([...current].filter((id) => loaded.has(id)));
      return next.size === current.size ? current : next;
    });
  }, [items]);
  const exitSelectionMode = () => {
    setSelectionMode(false);
    setSelectedEntityIds(new Set());
  };
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
  const selectionHeader = (
    <Checkbox
      checked={allLoadedSelected}
      disabled={items.length === 0}
      indeterminate={selectedItems.length > 0 && !allLoadedSelected}
      onChange={() =>
        setSelectedEntityIds(
          allLoadedSelected
            ? new Set()
            : new Set(
                items.slice(0, maximumAgentSelection).map((item) => item.id),
              ),
        )
      }
      slotProps={{
        input: { 'aria-label': t('explorer.selectLoadedEntities') },
      }}
    />
  );
  const selectionCheckbox = (entity: EntityItem) => (
    <Checkbox
      checked={selectedEntityIds.has(entity.id)}
      disabled={
        !selectedEntityIds.has(entity.id) &&
        selectedItems.length >= maximumAgentSelection
      }
      onChange={() =>
        setSelectedEntityIds((current) => {
          const next = new Set(current);
          if (next.has(entity.id)) next.delete(entity.id);
          else if (next.size < maximumAgentSelection) next.add(entity.id);
          return next;
        })
      }
      slotProps={{
        input: {
          'aria-label': t('explorer.selectEntity', {
            entity: displayLabel(entity.display, entity.id),
          }),
        },
      }}
    />
  );
  const columnDefinitions = buildExplorerColumnDefinitions({
    blueprint,
    tableColumns,
    publicationContextCode,
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
    ...(selectionMode ? [selectionColumn] : []),
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
        selectionMode={selectionMode}
        selectedCount={selectedItems.length}
        onClearSelection={() => setSelectedEntityIds(new Set())}
        onSendSelection={() => setAgentSelection([...selectedItems])}
        onToggleSelection={() =>
          selectionMode ? exitSelectionMode() : setSelectionMode(true)
        }
        onOpenColumnPreferences={() => setColumnPreferencesOpen(true)}
      />
      <TableContainer
        ref={tableContainerRef}
        sx={{
          // On desktop this leaves room for the sticky search form and result
          // summary while using the rest of the viewport for rows.
          height: {
            xs: 'calc(100dvh - 220px)',
            md: 'calc(100dvh - 165px)',
          },
          overflowY: 'auto',
        }}
      >
        {isFetching && items.length > 0 && (
          <LinearProgress
            aria-label={t('explorer.loading')}
            sx={{ position: 'sticky', top: 0, zIndex: 3 }}
          />
        )}
        <Table aria-label={t('explorer.results')} size="small" stickyHeader>
          <TableHead>
            {table.getHeaderGroups().map((group) => (
              <TableRow key={group.id}>
                {group.headers.map((header) => (
                  <TableCell
                    key={header.id}
                    sx={
                      header.column.id === 'actions'
                        ? {
                            bgcolor: 'background.paper',
                            boxShadow: 1,
                            position: 'sticky',
                            right: 0,
                            zIndex: 3,
                          }
                        : resultColumnCellSx(header.column.id)
                    }
                  >
                    {header.column.id === 'select'
                      ? selectionHeader
                      : header.isPlaceholder
                        ? null
                        : flexRender(
                            header.column.columnDef.header,
                            header.getContext(),
                          )}
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </TableHead>
          <TableBody>
            {paddingTop > 0 && (
              <TableRow>
                <TableCell
                  colSpan={columns.length}
                  sx={{ height: paddingTop, p: 0 }}
                />
              </TableRow>
            )}
            {virtualRows.map((virtualRow) => {
              if (virtualRow.index === rows.length) {
                return (
                  <TableRow data-index={virtualRow.index} key="load-more">
                    <TableCell colSpan={columns.length} sx={{ py: 2 }}>
                      <Box
                        sx={{
                          left: '50%',
                          position: 'sticky',
                          transform: 'translateX(-50%)',
                          width: 'fit-content',
                        }}
                      >
                        <LoadMoreButton
                          isLoading={isFetchingNextPage}
                          onLoadMore={onLoadMore}
                        />
                      </Box>
                    </TableCell>
                  </TableRow>
                );
              }
              const row = rows[virtualRow.index];
              return (
                <TableRow
                  data-index={virtualRow.index}
                  key={row.id}
                  ref={rowVirtualizer.measureElement}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell
                      key={cell.id}
                      sx={
                        cell.column.id === 'actions'
                          ? {
                              bgcolor: 'background.paper',
                              boxShadow: 1,
                              position: 'sticky',
                              right: 0,
                              zIndex: 1,
                            }
                          : resultColumnCellSx(cell.column.id)
                      }
                    >
                      {cell.column.id === 'select'
                        ? selectionCheckbox(row.original)
                        : flexRender(
                            cell.column.columnDef.cell,
                            cell.getContext(),
                          )}
                    </TableCell>
                  ))}
                </TableRow>
              );
            })}
            {paddingBottom > 0 && (
              <TableRow>
                <TableCell
                  colSpan={columns.length}
                  sx={{ height: paddingBottom, p: 0 }}
                />
              </TableRow>
            )}
          </TableBody>
        </Table>
      </TableContainer>
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
            setSelectedEntityIds((current) => {
              const next = new Set(current);
              next.delete(deleteEntityId);
              return next;
            });
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
            exitSelectionMode();
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
