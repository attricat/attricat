import {
  legacyCreateColumnHelper,
  type LegacyColumnDef,
} from '@tanstack/react-table/legacy';
import type { TFunction } from 'i18next';
import type {
  BlueprintWithAttributes,
  EntityItem,
  EntityPublicationStatus,
} from '../entities/api';
import { EntityIdPopover } from '../entities/components/EntityIdPopover';
import type { getExtensionRuntime } from '../extensions/api';
import { FilterableCell } from './CellFilterButton';
import type { AttributeFilterDraft } from './attributeFilterValues';
import {
  cellFilterDraft,
  cellFilterValueType,
  cellValueFilter,
} from './cellFilters';
import { ConfiguredColumnCell } from './ConfiguredColumnCell';
import {
  catalogRendererPrefix,
  embeddedExtensionKind,
  explorerColumnIds,
  explorerExtensionOutlets,
  explorerSortFields,
  explorerTableCellCapability,
  relationshipPathSeparator,
} from './constants';
import {
  ConfiguredColumnHeader,
  SortableColumnHeader,
} from './ExplorerColumnHeaders';
import {
  EntityActionsCell,
  EntityDisplayCell,
  PublicationStatusCell,
  SchemaVersionCell,
  type ActionMenuPosition,
} from './ExplorerTableCells';
import {
  configuredColumnLabel,
  usesExtensionRenderer,
  type ExplorerTableColumn,
} from './explorerTableColumns';
import type { ExplorerColumnPreferences } from './columnPreferences';
import type { AttributeFilter, ExplorerSort } from './search';

type ExtensionRuntime = Awaited<ReturnType<typeof getExtensionRuntime>>;
type ExplorerColumnDef = LegacyColumnDef<EntityItem, unknown>;

type ColumnOptions = {
  blueprint: BlueprintWithAttributes;
  tableColumns: ExplorerTableColumn[];
  publicationContextCode: string;
  publicationSortAvailable: boolean;
  publicationsByEntityId: Map<string, EntityPublicationStatus | undefined>;
  runtime: ExtensionRuntime | undefined;
  sort?: ExplorerSort;
  onSortChange: (field: string) => void;
  onFilterCell?: (draft: AttributeFilterDraft) => void;
  onOpenActions: (entityId: string, position: ActionMenuPosition) => void;
  takeCellFrame: () => boolean;
  t: TFunction;
  /** The user's effective zone, used to edit datetime cell filters. */
  timeZone: string;
};

const findTableCellExtension = (
  runtime: ExtensionRuntime | undefined,
  renderer: ExplorerTableColumn['renderer'],
) =>
  !renderer || renderer.id.startsWith(catalogRendererPrefix)
    ? undefined
    : runtime?.find(
        (item) =>
          item.outlet === explorerExtensionOutlets.tableCell &&
          item.kind === embeddedExtensionKind &&
          item.id === renderer.id &&
          item.version === renderer.version &&
          item.capabilities.includes(explorerTableCellCapability),
      );

// TanStack's column definitions are intentionally invariant in their value
// type. The table only consumes the shared row shape, so heterogeneous column
// values are normalized to `ExplorerColumnDef` at this boundary.
export const buildExplorerColumnDefinitions = ({
  blueprint,
  tableColumns,
  publicationContextCode,
  publicationSortAvailable,
  publicationsByEntityId,
  runtime,
  sort,
  onSortChange,
  onFilterCell,
  onOpenActions,
  takeCellFrame,
  t,
  timeZone,
}: ColumnOptions): ExplorerColumnDef[] => {
  const columnHelper = legacyCreateColumnHelper<EntityItem>();
  const attributes = new Map(
    blueprint.attributes.map((attribute) => [attribute.code, attribute]),
  );
  const configuredColumns = tableColumns.flatMap((column) => {
    const [relationship] = column.field.split(relationshipPathSeparator);
    const attribute = attributes.get(relationship);
    if (!attribute) return [];
    const extension = findTableCellExtension(runtime, column.renderer);
    const filterValueType = onFilterCell
      ? cellFilterValueType(
          column.field,
          attribute,
          blueprint.table_path_attributes,
        )
      : undefined;
    const filterFor = (entity: EntityItem) =>
      filterValueType
        ? cellValueFilter(
            column.field,
            filterValueType,
            entity.table_values[column.field] ?? [],
          )
        : undefined;
    const openFilter = (filter: AttributeFilter) =>
      filterValueType &&
      onFilterCell?.(cellFilterDraft(filter, filterValueType, timeZone));
    return [
      columnHelper.display({
        id: column.field,
        header: () => (
          <ConfiguredColumnHeader
            column={column}
            onSortChange={onSortChange}
            sort={sort}
          />
        ),
        // Frames are allocated while rendering visible cells only.
        cell: (info) => (
          <FilterableCell
            fieldLabel={configuredColumnLabel(column)}
            filter={filterFor(info.row.original)}
            onFilter={openFilter}
          >
            <ConfiguredColumnCell
              attribute={attribute}
              column={column}
              entity={info.row.original}
              extension={extension}
              frameAllowed={usesExtensionRenderer(column) && takeCellFrame()}
            />
          </FilterableCell>
        ),
      }) as ExplorerColumnDef,
    ];
  });
  return [
    columnHelper.accessor('id', {
      id: explorerColumnIds.id,
      header: t('explorer.id'),
      cell: (info) => <EntityIdPopover entityId={info.getValue()} />,
    }) as ExplorerColumnDef,
    columnHelper.display({
      id: explorerColumnIds.display,
      header: t('explorer.display'),
      cell: (info) => <EntityDisplayCell entity={info.row.original} />,
    }) as ExplorerColumnDef,
    columnHelper.display({
      id: explorerColumnIds.publication,
      header: () => {
        const label = t('explorer.publicationForContext', {
          context: publicationContextCode,
        });
        if (!publicationSortAvailable) return label;
        return (
          <SortableColumnHeader
            field={explorerSortFields.publicationStatus}
            label={label}
            onSortChange={onSortChange}
            sort={sort}
          />
        );
      },
      cell: (info) => (
        <PublicationStatusCell
          publication={publicationsByEntityId.get(info.row.original.id)}
        />
      ),
    }) as ExplorerColumnDef,
    columnHelper.display({
      id: explorerColumnIds.schema,
      header: () => (
        <SortableColumnHeader
          field={explorerSortFields.blueprintVersion}
          label={t('explorer.schema')}
          onSortChange={onSortChange}
          sort={sort}
        />
      ),
      cell: (info) => <SchemaVersionCell entity={info.row.original} />,
    }) as ExplorerColumnDef,
    ...configuredColumns,
    columnHelper.display({
      id: explorerColumnIds.actions,
      header: '',
      cell: (info) => (
        <EntityActionsCell
          entityId={info.row.original.id}
          onOpenActions={onOpenActions}
        />
      ),
    }) as ExplorerColumnDef,
  ];
};

/**
 * Orders and filters configurable columns by the viewer's preferences, keeping
 * the selection column first and the actions column last. Selection is
 * rendered outside the legacy table's memoized column definitions so checkbox
 * state follows selection changes even when result rows do not.
 */
export const arrangeExplorerColumns = (
  definitions: ExplorerColumnDef[],
  { hidden, order }: ExplorerColumnPreferences,
  selectionMode: boolean,
): ExplorerColumnDef[] => {
  const actionColumn = definitions.find(
    (definition) => definition.id === explorerColumnIds.actions,
  );
  const selectionColumn = legacyCreateColumnHelper<EntityItem>().display({
    id: explorerColumnIds.select,
    header: '',
    cell: () => null,
  }) as ExplorerColumnDef;
  return [
    ...(selectionMode ? [selectionColumn] : []),
    ...order.flatMap((id) => {
      const column = definitions.find((definition) => definition.id === id);
      return column && !hidden.includes(id) ? [column] : [];
    }),
    ...(actionColumn ? [actionColumn] : []),
  ];
};
