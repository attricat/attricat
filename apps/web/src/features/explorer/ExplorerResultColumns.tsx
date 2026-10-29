import {
  legacyCreateColumnHelper,
  type LegacyColumnDef,
} from '@tanstack/react-table/legacy';
import type { TFunction } from 'i18next';
import type { BlueprintWithAttributes, RecordItem } from '../records/api';
import { RecordIdPopover } from '../records/components/RecordIdPopover';
import type { getExtensionRuntime } from '../extensions/api';
import { contributionKinds, extensionOutlets } from '../extensions/constants';
import { FilterableCell } from './CellFilterButton';
import type { AttributeFilterDraft } from './attributeFilterValues';
import {
  cellFilterDraft,
  cellFilterValueType,
  cellValueFilter,
} from './cellFilters';
import { ConfiguredColumnCell } from './ConfiguredColumnCell';
import { ExplorerCellNotes } from '../extensions/ExplorerFieldNotes';
import {
  attricatRendererPrefix,
  explorerColumnIds,
  explorerSortFields,
  explorerTableCellCapability,
  relationshipPathSeparator,
} from './constants';
import {
  ConfiguredColumnHeader,
  SortableColumnHeader,
} from './ExplorerColumnHeaders';
import {
  RecordActionsCell,
  RecordDisplayCell,
  PublicationStatusCell,
  SchemaVersionCell,
  SystemTagsCell,
  type ActionMenuPosition,
  type OpenRecordPanel,
} from './ExplorerTableCells';
import {
  configuredColumnLabel,
  usesExtensionRenderer,
  type ExplorerTableColumn,
} from './explorerTableColumns';
import type { ExplorerColumnPreferences } from './columnPreferences';
import type { AttributeFilter } from './search';

type ExtensionRuntime = Awaited<ReturnType<typeof getExtensionRuntime>>;
type ExplorerColumnDef = LegacyColumnDef<RecordItem, unknown>;

type ColumnOptions = {
  blueprint: BlueprintWithAttributes;
  tableColumns: ExplorerTableColumn[];
  /** The selected context followed by its ancestors. */
  contextCodes: readonly string[];
  publicationContextCode: string;
  publicationContextId: string | undefined;
  publicationSortAvailable: boolean;
  runtime: ExtensionRuntime | undefined;
  onSortChange: (field: string) => void;
  onFilterCell?: (draft: AttributeFilterDraft) => void;
  onFilterSystemTag?: (tag: string) => void;
  onOpenActions: (recordId: string, position: ActionMenuPosition) => void;
  onOpenPanel?: OpenRecordPanel;
  onPrefetchPanel?: (record: RecordItem) => void;
  takeCellFrame: () => boolean;
  t: TFunction;
  /** The user's effective zone, used to edit datetime cell filters. */
  timeZone: string;
};

const findTableCellExtension = (
  runtime: ExtensionRuntime | undefined,
  renderer: ExplorerTableColumn['renderer'],
) =>
  !renderer || renderer.id.startsWith(attricatRendererPrefix)
    ? undefined
    : runtime?.find(
        (item) =>
          item.outlet === extensionOutlets.explorerTableCell &&
          item.kind === contributionKinds.embedded &&
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
  contextCodes,
  publicationContextCode,
  publicationContextId,
  publicationSortAvailable,
  runtime,
  onSortChange,
  onFilterCell,
  onFilterSystemTag,
  onOpenActions,
  onOpenPanel,
  onPrefetchPanel,
  takeCellFrame,
  t,
  timeZone,
}: ColumnOptions): ExplorerColumnDef[] => {
  const columnHelper = legacyCreateColumnHelper<RecordItem>();
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
    const filterFor = (record: RecordItem) =>
      filterValueType
        ? cellValueFilter(
            column.field,
            filterValueType,
            record.table_values[column.field] ?? [],
          )
        : undefined;
    const openFilter = (filter: AttributeFilter) =>
      filterValueType &&
      onFilterCell?.(cellFilterDraft(filter, filterValueType, timeZone));
    return [
      columnHelper.display({
        id: column.field,
        header: () => (
          <ConfiguredColumnHeader column={column} onSortChange={onSortChange} />
        ),
        // Frames are allocated while rendering visible cells only.
        cell: (info) => (
          <FilterableCell
            fieldLabel={configuredColumnLabel(column)}
            filter={filterFor(info.row.original)}
            onFilter={openFilter}
          >
            <ExplorerCellNotes
              attributeCode={column.field}
              recordId={info.row.original.id}
            >
              <ConfiguredColumnCell
                attribute={attribute}
                column={column}
                record={info.row.original}
                extension={extension}
                frameAllowed={usesExtensionRenderer(column) && takeCellFrame()}
              />
            </ExplorerCellNotes>
          </FilterableCell>
        ),
      }) as ExplorerColumnDef,
    ];
  });
  return [
    columnHelper.accessor('id', {
      id: explorerColumnIds.id,
      header: t('explorer.id'),
      cell: (info) => <RecordIdPopover recordId={info.getValue()} />,
    }) as ExplorerColumnDef,
    columnHelper.display({
      id: explorerColumnIds.display,
      header: t('explorer.display'),
      cell: (info) => (
        <RecordDisplayCell
          contextCodes={contextCodes}
          record={info.row.original}
          onOpenPanel={onOpenPanel}
          onPrefetchPanel={onPrefetchPanel}
        />
      ),
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
          />
        );
      },
      cell: (info) => (
        <PublicationStatusCell
          contextId={publicationContextId}
          recordId={info.row.original.id}
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
        />
      ),
      cell: (info) => <SchemaVersionCell record={info.row.original} />,
    }) as ExplorerColumnDef,
    columnHelper.display({
      id: explorerColumnIds.systemTags,
      header: t('explorer.systemTags'),
      cell: (info) => (
        <SystemTagsCell
          onFilterTag={onFilterSystemTag}
          record={info.row.original}
        />
      ),
    }) as ExplorerColumnDef,
    ...configuredColumns,
    columnHelper.display({
      id: explorerColumnIds.actions,
      header: '',
      cell: (info) => (
        <RecordActionsCell
          recordId={info.row.original.id}
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
  const selectionColumn = legacyCreateColumnHelper<RecordItem>().display({
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
