import type { TFunction } from 'i18next';
import type { BlueprintWithAttributes } from '../entities/api';
import { attributeLabel } from '../entities/entityDisplay';
import type { AttributeValueType } from '../entities/valueTypeIcons';
import {
  builtInConfigurableColumnIds,
  catalogRendererPrefix,
  explorerColumnIds,
  relationshipPathSeparator,
} from './constants';
import { lexiconText } from '../lexicon/lexicon';

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

export const isRelationshipPath = (field: string) =>
  field.includes(relationshipPathSeparator);

export const usesExtensionRenderer = (column: ExplorerTableColumn) =>
  Boolean(
    column.renderer && !column.renderer.id.startsWith(catalogRendererPrefix),
  );

export const humanizeField = (field: string) => field.replaceAll('_', ' ');

export const configuredColumnLabel = (column: ExplorerTableColumn) =>
  column.label ?? humanizeField(column.field);

/** Resolves the blueprint's table view into sortable Explorer columns. */
export const buildExplorerTableColumns = (
  blueprint: BlueprintWithAttributes,
  relationshipSortAvailable: boolean,
): ExplorerTableColumn[] => {
  // Local columns default to the attribute's name; relationship paths keep
  // the humanized path.
  const defaultLabel = (field: string) => {
    const attribute = blueprint.attributes.find(
      (candidate) => candidate.code === field,
    );
    return attribute ? attributeLabel(attribute) : undefined;
  };
  const tableView =
    blueprint.blueprint.views.table?.type === 'table'
      ? blueprint.blueprint.views.table
      : undefined;
  if (!tableView?.columns?.length)
    return (tableView?.fields ?? []).map((field) => ({
      field,
      label: defaultLabel(field),
      relationshipSortBlocked: false,
      sortable: false,
    }));
  return tableView.columns.map((column) => {
    const configuredSortable =
      blueprint.table_path_attributes.find(
        (attribute) => attribute.code === column.field,
      )?.sortable ?? false;
    const relationshipSortBlocked =
      configuredSortable &&
      isRelationshipPath(column.field) &&
      !relationshipSortAvailable;
    return {
      ...column,
      label: column.label
        ? lexiconText(column.label)
        : defaultLabel(column.field),
      relationshipSortBlocked,
      sortable: configuredSortable && !relationshipSortBlocked,
    };
  });
};

/** The value type a column shows, or undefined for built-in columns. */
export const explorerColumnValueType = (
  blueprint: BlueprintWithAttributes,
  id: string,
): AttributeValueType | undefined =>
  (
    blueprint.attributes.find((attribute) => attribute.code === id) ??
    blueprint.table_path_attributes.find((attribute) => attribute.code === id)
  )?.value_type;

export const configurableColumnIds = (tableColumns: ExplorerTableColumn[]) => [
  ...builtInConfigurableColumnIds,
  ...tableColumns.map((column) => column.field),
];

export const explorerColumnLabel = (
  t: TFunction,
  id: string,
  tableColumns: ExplorerTableColumn[],
  publicationContextCode: string,
) => {
  if (id === explorerColumnIds.id) return t('explorer.id');
  if (id === explorerColumnIds.display) return t('explorer.display');
  if (id === explorerColumnIds.publication)
    return t('explorer.publicationForContext', {
      context: publicationContextCode,
    });
  if (id === explorerColumnIds.schema) return t('explorer.schema');
  return (
    tableColumns.find((column) => column.field === id)?.label ??
    humanizeField(id)
  );
};
