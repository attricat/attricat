import { Link } from '@tanstack/react-router';
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
import MoreVertIcon from '@mui/icons-material/MoreVert';
import {
  Alert,
  ButtonBase,
  Chip,
  Dialog,
  DialogContent,
  DialogTitle,
  IconButton,
  LinearProgress,
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
} from '@mui/material';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  getEntityPublications,
  publishEntity,
  unpublishEntity,
  type BlueprintWithAttributes,
  type EntityItem,
  type EntityPublicationStatus,
} from '../entities/api';
import { EntityIdPopover } from '../entities/components/EntityIdPopover';
import { displayLabel } from '../entities/entity-display';
import { AttributeValue } from '../views/components/values/AttributeValue';
import { ExtensionPopoverOutlet } from '../extensions/ExtensionOutlet';
import { getExtensionRuntime } from '../extensions/api';
import { extensionQueryKeys } from '../extensions/query-keys';
import { extensionRuntimeRefetchInterval } from '../extensions/constants';
import { ExtensionTableCell } from './ExtensionTableCell';
import { ImageTableCell } from './ImageTableCell';
import { explorerTableCellContextSchema } from './schemas';
import { entityQueryKeys } from '../entities/query-keys';

const maximumExplorerCellFrames = 32;

const EntityActionsMenu = ({
  blueprintId,
  canPublish,
  entity,
  onSearchInfo,
  publication,
  publicationContextId,
  publish,
  publishing,
  unpublish,
  unpublishing,
}: {
  blueprintId: string;
  canPublish: boolean;
  entity: EntityItem;
  onSearchInfo: (entity: EntityItem) => void;
  publication: EntityPublicationStatus | undefined;
  publicationContextId: string | undefined;
  publish: () => void;
  publishing: boolean;
  unpublish: () => void;
  unpublishing: boolean;
}) => {
  const { t } = useTranslation();
  const [position, setPosition] = useState<{
    left: number;
    top: number;
  } | null>(null);

  return (
    <>
      <IconButton
        aria-label={t('explorer.entityActionsFor', { entityId: entity.id })}
        onClick={(event) => {
          const { left, top } = event.currentTarget.getBoundingClientRect();
          setPosition({ left, top });
        }}
        size="small"
      >
        <MoreVertIcon fontSize="inherit" />
      </IconButton>
      <Menu
        anchorPosition={position ?? undefined}
        anchorReference="anchorPosition"
        onClose={() => setPosition(null)}
        open={Boolean(position)}
        transformOrigin={{ horizontal: 'right', vertical: 'top' }}
      >
        <MenuItem
          onClick={() => {
            setPosition(null);
            onSearchInfo(entity);
          }}
        >
          {t('explorer.searchInfo')}
        </MenuItem>
        {canPublish && publicationContextId && publication && (
          <>
            {publication.status === 'not_published' ? (
              <MenuItem
                disabled={publishing}
                onClick={() => {
                  setPosition(null);
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
                    setPosition(null);
                    publish();
                  }}
                >
                  {t('entities.publish')}
                </MenuItem>
                <MenuItem
                  disabled={unpublishing}
                  onClick={() => {
                    setPosition(null);
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
    </>
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
  relationshipSortAvailable?: boolean;
  sort?: { field: string; direction: 'asc' | 'desc' };
  totalCount: number | null;
  totalCountCapped: boolean;
}) => {
  const { t } = useTranslation();
  const [searchInfoEntity, setSearchInfoEntity] = useState<EntityItem | null>(
    null,
  );
  const queryClient = useQueryClient();
  const publicationQueries = useQueries({
    queries: items.map((entity) => ({
      queryKey: entityQueryKeys.publications(entity.id),
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
      entityQueryKeys.publications(entityId),
      (current) => [
        ...(current ?? []).filter(
          (item) => item.context_id !== publication.context_id,
        ),
        publication,
      ],
    );
  const invalidatePublication = (entityId: string) =>
    queryClient.invalidateQueries({
      queryKey: entityQueryKeys.publications(entityId),
    });
  const publish = useMutation({
    mutationFn: (entityId: string) =>
      publishEntity(entityId, publicationContextId!),
    onSuccess: (publication, entityId) => {
      updatePublication(entityId, publication);
      void invalidatePublication(entityId);
    },
  });
  const unpublish = useMutation({
    mutationFn: (entityId: string) =>
      unpublishEntity(entityId, publicationContextId!),
    onSuccess: (_, entityId) => void invalidatePublication(entityId),
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
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime(),
    queryFn: getExtensionRuntime,
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
  const columns: LegacyColumnDef<EntityItem, string>[] = [
    columnHelper.accessor('id', {
      header: t('explorer.id'),
      cell: (info) => <EntityIdPopover entityId={info.getValue()} />,
    }),
    columnHelper.display({
      id: 'display',
      header: t('explorer.display'),
      cell: (info) => (
        <Link
          params={{ entityId: info.row.original.id }}
          to="/entities/$entityId"
        >
          {displayLabel(info.row.original.display, info.row.original.id)}
        </Link>
      ),
    }) as LegacyColumnDef<EntityItem, string>,
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
    }) as LegacyColumnDef<EntityItem, string>,
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
    }) as LegacyColumnDef<EntityItem, string>,
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
        }) as LegacyColumnDef<EntityItem, string>,
      ];
    }),
    columnHelper.display({
      id: 'actions',
      header: '',
      cell: (info) => {
        const entity = info.row.original;
        return (
          <EntityActionsMenu
            blueprintId={String(blueprint.blueprint.id)}
            canPublish={canPublish}
            entity={entity}
            onSearchInfo={setSearchInfoEntity}
            publication={publicationsByEntityId.get(entity.id)}
            publicationContextId={publicationContextId}
            publish={() => publish.mutate(entity.id)}
            publishing={publish.isPending}
            unpublish={() => unpublish.mutate(entity.id)}
            unpublishing={unpublish.isPending}
          />
        );
      },
    }) as LegacyColumnDef<EntityItem, string>,
  ];
  const table = useLegacyTable({
    data: items,
    columns: columns as never,
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

  return (
    <Paper component="section">
      {(publish.isError || unpublish.isError) && (
        <Alert severity="error" sx={{ m: 2 }}>
          {publish.error?.message ?? unpublish.error?.message}
        </Alert>
      )}
      <Typography sx={{ borderBottom: 1, borderColor: 'divider', p: 2 }}>
        {totalCount === null
          ? t('explorer.resultCount', { count: items.length })
          : totalCountCapped
            ? t('explorer.resultCountCapped', { count: totalCount })
            : t('explorer.resultCount', { count: totalCount })}
      </Typography>
      <TableContainer
        aria-label={t('explorer.results')}
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
        <Table size="small" stickyHeader>
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
                        : header.column.id === 'display'
                          ? { minWidth: 280 }
                          : header.column.id === 'id'
                            ? { textAlign: 'center', width: 48 }
                            : {}
                    }
                  >
                    {header.isPlaceholder
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
                    <TableCell
                      colSpan={columns.length}
                      sx={{ py: 2, textAlign: 'center' }}
                    >
                      <LoadMoreButton
                        isLoading={isFetchingNextPage}
                        onLoadMore={onLoadMore}
                      />
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
                          : cell.column.id === 'display'
                            ? { minWidth: 280 }
                            : cell.column.id === 'id'
                              ? { textAlign: 'center', width: 48 }
                              : {}
                      }
                    >
                      {flexRender(
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
