import { Link } from '@tanstack/react-router';
import {
  legacyCreateColumnHelper,
  type LegacyColumnDef,
} from '@tanstack/react-table/legacy';
import MoreVertIcon from '@mui/icons-material/MoreVert';
import {
  Box,
  ButtonBase,
  Chip,
  IconButton,
  TableSortLabel,
  Tooltip,
  Typography,
} from '@mui/material';
import type { TFunction } from 'i18next';
import type {
  BlueprintWithAttributes,
  EntityItem,
  EntityPublicationStatus,
} from '../entities/api';
import { EntityIdPopover } from '../entities/components/EntityIdPopover';
import { displayLabel } from '../entities/entity-display';
import { AttributeValue } from '../views/components/values/AttributeValue';
import type { getExtensionRuntime } from '../extensions/api';
import { ExtensionTableCell } from './ExtensionTableCell';
import { ImageTableCell } from './ImageTableCell';
import { explorerTableCellContextSchema } from './schemas';

export type ExplorerTableColumn = {
  field: string;
  label?: string | null;
  renderer?: {
    id: string;
    version: number;
    props: Record<string, unknown>;
  } | null;
  relationshipSortBlocked: boolean;
  sortable: boolean;
};

type ColumnOptions = {
  blueprint: BlueprintWithAttributes;
  tableColumns: ExplorerTableColumn[];
  publicationContextCode: string;
  publicationsByEntityId: Map<string, EntityPublicationStatus | undefined>;
  runtime: Awaited<ReturnType<typeof getExtensionRuntime>> | undefined;
  sort?: { field: string; direction: 'asc' | 'desc' };
  onSortChange: (field: string) => void;
  onOpenActions: (
    entityId: string,
    position: { left: number; top: number },
  ) => void;
  takeCellFrame: () => boolean;
  t: TFunction;
};

export const buildExplorerColumnDefinitions = ({
  blueprint,
  tableColumns,
  publicationContextCode,
  publicationsByEntityId,
  runtime,
  sort,
  onSortChange,
  onOpenActions,
  takeCellFrame,
  t,
}: ColumnOptions): LegacyColumnDef<EntityItem, unknown>[] => {
  const columnHelper = legacyCreateColumnHelper<EntityItem>();
  const attributes = new Map(
    blueprint.attributes.map((attribute) => [attribute.code, attribute]),
  );
  return [
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
        : runtime?.find(
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
              onOpenActions(entity.id, { left, top });
            }}
            size="small"
          >
            <MoreVertIcon fontSize="inherit" />
          </IconButton>
        );
      },
    }) as LegacyColumnDef<EntityItem, unknown>,
  ];
};
