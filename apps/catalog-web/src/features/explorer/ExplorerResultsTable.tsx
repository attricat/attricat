import { Link, useNavigate } from '@tanstack/react-router';
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
import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import MoreVertIcon from '@mui/icons-material/MoreVert';
import SettingsIcon from '@mui/icons-material/Settings';
import {
  Alert,
  Box,
  Button,
  ButtonBase,
  Checkbox,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  LinearProgress,
  List,
  ListItem,
  ListItemText,
  Menu,
  MenuItem,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TableSortLabel,
  Tooltip,
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
import { EntityIdPopover } from '../entities/components/EntityIdPopover';
import { DeleteEntityDialog } from '../entities/components/DeleteEntityDialog';
import { displayLabel } from '../entities/entity-display';
import { AttributeValue } from '../views/components/values/AttributeValue';
import { ExtensionPopoverOutlet } from '../extensions/ExtensionOutlet';
import { getExtensionRuntime } from '../extensions/api';
import { extensionQueryKeys } from '../extensions/query-keys';
import { extensionRuntimeRefetchInterval } from '../extensions/constants';
import { ExtensionTableCell } from './ExtensionTableCell';
import { ImageTableCell } from './ImageTableCell';
import { maximumAgentSelection } from './agent-selection';
import { SendSelectedToAgentDialog } from './SendSelectedToAgentDialog';
import { explorerTableCellContextSchema } from './schemas';
import { entityQueryKeys } from '../entities/query-keys';
import {
  clearExplorerColumnPreferences,
  getExplorerColumnPreferences,
  setExplorerColumnPreferences,
  type ExplorerColumnPreferences,
} from './column-preferences';

const maximumExplorerCellFrames = 32;
const columnPreferencesListMaxHeight = 480;

const resultColumnCellSx = (columnId: string): SxProps<Theme> =>
  columnId === 'display'
    ? { minWidth: 280 }
    : columnId === 'id'
      ? { textAlign: 'center', width: 48 }
      : {};

const EntityActionsMenu = ({
  blueprintId,
  canPublish,
  canDelete,
  entity,
  onClose,
  onDelete,
  onSearchInfo,
  position,
  publication,
  publicationContextId,
  publish,
  publishing,
  duplicate,
  duplicating,
  unpublish,
  unpublishing,
}: {
  blueprintId: string;
  canPublish: boolean;
  canDelete: boolean;
  entity: EntityItem;
  onClose: () => void;
  onDelete: () => void;
  onSearchInfo: (entity: EntityItem) => void;
  position: { left: number; top: number } | null;
  publication: EntityPublicationStatus | undefined;
  publicationContextId: string | undefined;
  publish: () => void;
  publishing: boolean;
  duplicate: () => void;
  duplicating: boolean;
  unpublish: () => void;
  unpublishing: boolean;
}) => {
  const { t } = useTranslation();

  return (
    <Menu
      anchorPosition={position ?? undefined}
      anchorReference="anchorPosition"
      onClose={onClose}
      open={Boolean(position)}
      transformOrigin={{ horizontal: 'right', vertical: 'top' }}
    >
      <MenuItem
        onClick={() => {
          onClose();
          onSearchInfo(entity);
        }}
      >
        {t('explorer.searchInfo')}
      </MenuItem>
      <MenuItem
        disabled={duplicating}
        onClick={() => {
          onClose();
          duplicate();
        }}
      >
        {t('entities.duplicateEntity')}
      </MenuItem>
      {canDelete && (
        <MenuItem
          onClick={() => {
            onClose();
            onDelete();
          }}
        >
          {t('entities.deleteEntity')}
        </MenuItem>
      )}
      {canPublish && publicationContextId && publication && (
        <>
          {publication.status === 'not_published' ? (
            <MenuItem
              disabled={publishing}
              onClick={() => {
                onClose();
                publish();
              }}
            >
              {t('entities.publish')}
            </MenuItem>
          ) : (
            <>
              <MenuItem
                disabled={publishing}
                onClick={() => {
                  onClose();
                  publish();
                }}
              >
                {t('entities.publish')}
              </MenuItem>
              <MenuItem
                disabled={unpublishing}
                onClick={() => {
                  onClose();
                  unpublish();
                }}
              >
                {t('entities.unpublish')}
              </MenuItem>
            </>
          )}
        </>
      )}
      <ExtensionPopoverOutlet
        context={{
          context_version: 1,
          blueprint_id: blueprintId,
          blueprint_version: entity.blueprint_version,
          entity_id: entity.id,
        }}
        label={t('explorer.extensionActions')}
        outlet="explorer_row_action"
      />
    </Menu>
  );
};

const ExplorerColumnPreferencesDialog = ({
  columns,
  onChange,
  onClear,
  onClose,
  open,
  preferences,
}: {
  columns: { id: string; label: string }[];
  onChange: (preferences: ExplorerColumnPreferences) => void;
  onClear: () => void;
  onClose: () => void;
  open: boolean;
  preferences: ExplorerColumnPreferences;
}) => {
  const { t } = useTranslation();
  const labels = new Map(columns.map((column) => [column.id, column.label]));
  const move = (id: string, direction: -1 | 1) => {
    const index = preferences.order.indexOf(id);
    const target = index + direction;
    if (target < 0 || target >= preferences.order.length) return;
    const order = [...preferences.order];
    [order[index], order[target]] = [order[target], order[index]];
    onChange({ ...preferences, order });
  };

  return (
    <Dialog fullWidth maxWidth="sm" onClose={onClose} open={open}>
      <DialogTitle>{t('explorer.columnPreferences')}</DialogTitle>
      <DialogContent>
        <Box
          sx={{ maxHeight: columnPreferencesListMaxHeight, overflowY: 'auto' }}
        >
          <List aria-label={t('explorer.columns')}>
            {preferences.order.map((id, index) => (
              <ListItem
                key={id}
                secondaryAction={
                  <>
                    <Tooltip title={t('explorer.moveColumnUp')}>
                      <span>
                        <IconButton
                          aria-label={t('explorer.moveColumnUp')}
                          disabled={index === 0}
                          onClick={() => move(id, -1)}
                          size="small"
                        >
                          <ArrowUpwardIcon />
                        </IconButton>
                      </span>
                    </Tooltip>
                    <Tooltip title={t('explorer.moveColumnDown')}>
                      <span>
                        <IconButton
                          aria-label={t('explorer.moveColumnDown')}
                          disabled={index === preferences.order.length - 1}
                          onClick={() => move(id, 1)}
                          size="small"
                        >
                          <ArrowDownwardIcon />
                        </IconButton>
                      </span>
                    </Tooltip>
                  </>
                }
              >
                <Checkbox
                  checked={!preferences.hidden.includes(id)}
                  edge="start"
                  slotProps={{
                    input: {
                      'aria-label': t('explorer.showColumn', {
                        column: labels.get(id),
                      }),
                    },
                  }}
                  onChange={() =>
                    onChange({
                      ...preferences,
                      hidden: preferences.hidden.includes(id)
                        ? preferences.hidden.filter((hidden) => hidden !== id)
                        : [...preferences.hidden, id],
                    })
                  }
                />
                <ListItemText primary={labels.get(id) ?? id} />
              </ListItem>
            ))}
          </List>
        </Box>
      </DialogContent>
      <DialogActions>
        <Button color="inherit" onClick={onClear}>
          {t('explorer.clearColumnPreferences')}
        </Button>
        <Button onClick={onClose} variant="contained">
          {t('explorer.done')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};

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
  const tableColumns: {
    field: string;
    label?: string | null;
    renderer?: {
      id: string;
      version: number;
      props: Record<string, unknown>;
    } | null;
    relationshipSortBlocked: boolean;
    sortable: boolean;
  }[] = tableView?.columns?.length
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
  const attributes = new Map(
    blueprint.attributes.map((attribute) => [attribute.code, attribute]),
  );
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
  const columnDefinitions: LegacyColumnDef<EntityItem, unknown>[] = [
    // TanStack's column definitions are intentionally invariant in their
    // value type. The table only consumes the shared row shape, so normalize
    // heterogeneous column values at this boundary.
    columnHelper.accessor('id', {
      id: 'id',
      header: t('explorer.id'),
      cell: (info) => <EntityIdPopover entityId={info.getValue()} />,
    }) as LegacyColumnDef<EntityItem, unknown>,
    columnHelper.display({
      id: 'display',
      header: t('explorer.display'),
      cell: (info) => (
        <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
          <Link
            params={{ entityId: info.row.original.id }}
            to="/entities/$entityId"
          >
            {displayLabel(info.row.original.display, info.row.original.id)}
          </Link>
          {info.row.original.is_sample && (
            <Chip color="info" label={t('entities.sample')} size="small" />
          )}
        </Box>
      ),
    }) as LegacyColumnDef<EntityItem, unknown>,
    columnHelper.display({
      id: 'publication',
      header: t('explorer.publicationForContext', {
        context: publicationContextCode,
      }),
      cell: (info) => {
        const publication = publicationsByEntityId.get(info.row.original.id);
        if (!publication) return '—';
        return (
          <Chip
            color={publication.status === 'published' ? 'success' : 'default'}
            label={t(`entities.publication.${publication.status}`)}
            size="small"
          />
        );
      },
    }) as LegacyColumnDef<EntityItem, unknown>,
    columnHelper.display({
      id: 'schema',
      header: t('explorer.schema'),
      cell: (info) => {
        const entity = info.row.original;
        return (
          <Chip
            color={entity.schema_outdated ? 'warning' : 'success'}
            label={`v${entity.blueprint_version} · ${
              entity.schema_outdated
                ? t('explorer.outdated')
                : t('explorer.current')
            }`}
            size="small"
          />
        );
      },
    }) as LegacyColumnDef<EntityItem, unknown>,
    ...tableColumns.flatMap((column) => {
      const [relationship] = column.field.split('.');
      const relatedPath = column.field.includes('.');
      const attribute = attributes.get(relationship);
      if (!attribute) return [];
      const renderer = column.renderer;
      const extension = renderer?.id.startsWith('catalog.')
        ? undefined
        : runtime.data?.find(
            (item) =>
              item.outlet === 'explorer_table_cell' &&
              item.kind === 'embedded' &&
              item.id === renderer?.id &&
              item.version === renderer.version &&
              item.capabilities.includes('client.explorer_table_cell'),
          );
      return [
        columnHelper.display({
          id: column.field,
          header: () => {
            const label = column.label ?? column.field.replaceAll('_', ' ');
            if (column.relationshipSortBlocked)
              return (
                <Tooltip
                  title={t('explorer.relationshipSortNeedsSingleVersion')}
                >
                  <ButtonBase
                    aria-disabled="true"
                    aria-label={`${label}. ${t('explorer.relationshipSortNeedsSingleVersion')}`}
                    disableRipple
                    sx={{ cursor: 'help', font: 'inherit' }}
                  >
                    {label}
                  </ButtonBase>
                </Tooltip>
              );
            if (!column.sortable) return label;
            const active = sort?.field === column.field;
            return (
              <TableSortLabel
                active={active}
                direction={active ? sort.direction : 'asc'}
                onClick={() => onSortChange(column.field)}
              >
                {label}
              </TableSortLabel>
            );
          },
          cell: (info) => {
            const entity = info.row.original;
            const related = relatedPath
              ? entity.related?.[relationship]?.[0]
              : undefined;
            const pathValues = entity.table_values[column.field] ?? [];
            const primaryValue =
              pathValues.length > 1 ? pathValues : pathValues[0];
            // Search projections contain the related target's scalar but not
            // its attribute definition. Keep that rendering deliberately
            // defensive: an absent relation or incompatible value is not an
            // excuse to fetch a row (or to break virtualized rendering).
            const fallback = relatedPath ? (
              <Typography
                color={
                  primaryValue === null || primaryValue === undefined
                    ? 'text.secondary'
                    : undefined
                }
                variant="body2"
              >
                {primaryValue === null || primaryValue === undefined
                  ? t('views.notSet')
                  : Array.isArray(primaryValue)
                    ? primaryValue
                        .map((value) =>
                          typeof value === 'object'
                            ? JSON.stringify(value)
                            : String(value),
                        )
                        .join(', ')
                    : typeof primaryValue === 'object'
                      ? JSON.stringify(primaryValue)
                      : String(primaryValue)}
              </Typography>
            ) : (
              <AttributeValue
                attribute={attribute}
                compact
                value={primaryValue}
              />
            );
            if (renderer?.id === 'catalog.table_image')
              return <ImageTableCell value={primaryValue} />;
            if (!renderer || renderer.id.startsWith('catalog.'))
              return fallback;
            const context = explorerTableCellContextSchema.parse({
              context_version: 1,
              column: {
                field: column.field,
                label: column.label ?? null,
                renderer,
              },
              primary_value: primaryValue ?? null,
              related_entity: related
                ? {
                    id: related.id,
                    blueprint_id: related.blueprint_id,
                    blueprint_version: related.blueprint_version,
                    relationship_context_id: related.relationship_context_id,
                    relationship_context_code:
                      related.relationship_context_code,
                  }
                : null,
              related_preview: related?.preview ?? null,
              source_row: {
                entity_id: entity.id,
                blueprint_version: entity.blueprint_version,
                preview: entity.preview,
              },
            });
            return (
              <ExtensionTableCell
                context={context}
                contribution={extension}
                key={extension?.release_id}
                fallback={fallback}
                frameAllowed={takeCellFrame()}
              />
            );
          },
        }) as LegacyColumnDef<EntityItem, unknown>,
      ];
    }),
    columnHelper.display({
      id: 'actions',
      header: '',
      cell: (info) => {
        const entity = info.row.original;
        return (
          <IconButton
            aria-label={t('explorer.entityActionsFor', { entityId: entity.id })}
            onClick={(event) => {
              const { left, top } = event.currentTarget.getBoundingClientRect();
              setActionMenu({ entityId: entity.id, position: { left, top } });
            }}
            size="small"
          >
            <MoreVertIcon fontSize="inherit" />
          </IconButton>
        );
      },
    }) as LegacyColumnDef<EntityItem, unknown>,
  ];
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
      <Box
        sx={{
          alignItems: 'center',
          borderBottom: 1,
          borderColor: 'divider',
          display: 'flex',
          justifyContent: 'space-between',
          p: 1,
          pl: 2,
        }}
      >
        <Box
          sx={{
            alignItems: 'center',
            display: 'flex',
            flexWrap: 'wrap',
            gap: 1,
          }}
        >
          <Typography>
            {totalCount === null
              ? t('explorer.resultCount', { count: items.length })
              : totalCountCapped
                ? t('explorer.resultCountCapped', { count: totalCount })
                : t('explorer.resultCount', { count: totalCount })}
          </Typography>
          {selectionMode && selectedItems.length > 0 && (
            <>
              <Typography>
                {t('explorer.selectedCount', { count: selectedItems.length })}
              </Typography>
              <Button
                onClick={() => setSelectedEntityIds(new Set())}
                size="small"
              >
                {t('explorer.clearSelection')}
              </Button>
              <Button
                onClick={() => setAgentSelection([...selectedItems])}
                size="small"
                variant="contained"
              >
                {t('explorer.sendToAgentConversation')}
              </Button>
            </>
          )}
          {selectionMode && (
            <Typography variant="caption">
              {t('explorer.selectionLimit', { count: maximumAgentSelection })}
            </Typography>
          )}
        </Box>
        <Box sx={{ alignItems: 'center', display: 'flex' }}>
          <Button
            onClick={() =>
              selectionMode ? exitSelectionMode() : setSelectionMode(true)
            }
            size="small"
          >
            {t(
              selectionMode
                ? 'explorer.exitSelection'
                : 'explorer.selectEntities',
            )}
          </Button>
          <Tooltip title={t('explorer.columnPreferences')}>
            <IconButton
              aria-label={t('explorer.columnPreferences')}
              onClick={() => setColumnPreferencesOpen(true)}
            >
              <SettingsIcon />
            </IconButton>
          </Tooltip>
        </Box>
      </Box>
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
