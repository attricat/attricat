import { createElement } from 'react';
import { Typography } from '@mui/material';
import type { Attribute, RecordItem } from '../records/api';
import type { ExtensionContribution } from '../extensions/api';
import { resolveValueRenderer } from '../views/components/registry';
import { AttributeValue } from '../views/components/values/AttributeValue';
import { NotSetValue } from '../views/components/values/NotSetValue';
import {
  explorerExtensionContextVersion,
  relationshipPathSeparator,
} from './constants';
import {
  isRelationshipPath,
  usesExtensionRenderer,
  type ExplorerTableColumn,
} from './explorerTableColumns';
import { ExtensionTableCell } from './ExtensionTableCell';
import { explorerTableCellContextSchema } from './schemas';

const formatProjectedValue = (value: unknown) =>
  typeof value === 'object' ? JSON.stringify(value) : String(value);

/**
 * Search projections contain the related target's scalar but not its
 * attribute definition. Keep that rendering deliberately defensive: an absent
 * relation or incompatible value is not an excuse to fetch a row (or to break
 * virtualized rendering).
 */
const RelatedPathValue = ({ value }: { value: unknown }) =>
  value === null || value === undefined ? (
    <NotSetValue />
  ) : (
    <Typography variant="body2">
      {Array.isArray(value)
        ? value.map(formatProjectedValue).join(', ')
        : formatProjectedValue(value)}
    </Typography>
  );

type Props = {
  attribute: Attribute;
  column: ExplorerTableColumn;
  record: RecordItem;
  extension: ExtensionContribution | undefined;
  frameAllowed: boolean;
};

export const ConfiguredColumnCell = ({
  attribute,
  column,
  record,
  extension,
  frameAllowed,
}: Props) => {
  const [relationship] = column.field.split(relationshipPathSeparator);
  const relatedPath = isRelationshipPath(column.field);
  const { renderer } = column;
  const related = relatedPath ? record.related?.[relationship]?.[0] : undefined;
  const pathValues = record.table_values[column.field] ?? [];
  const primaryValue = pathValues.length > 1 ? pathValues : pathValues[0];
  const fallback = relatedPath ? (
    <RelatedPathValue value={primaryValue} />
  ) : (
    <AttributeValue attribute={attribute} compact value={primaryValue} />
  );
  const Renderer = resolveValueRenderer(renderer);
  if (Renderer)
    return createElement(Renderer, {
      attribute,
      component: renderer,
      value: primaryValue,
    });
  if (!renderer || !usesExtensionRenderer(column)) return fallback;
  const context = explorerTableCellContextSchema.parse({
    context_version: explorerExtensionContextVersion,
    column: {
      field: column.field,
      label: column.label ?? null,
      renderer,
    },
    primary_value: primaryValue ?? null,
    related_record: related
      ? {
          id: related.id,
          blueprint_id: related.blueprint_id,
          blueprint_version: related.blueprint_version,
          relationship_context_id: related.relationship_context_id,
          relationship_context_code: related.relationship_context_code,
        }
      : null,
    related_preview: related?.preview ?? null,
    source_row: {
      record_id: record.id,
      blueprint_version: record.blueprint_version,
      preview: record.preview,
    },
  });
  return (
    <ExtensionTableCell
      context={context}
      contribution={extension}
      key={extension?.release_id}
      fallback={fallback}
      frameAllowed={frameAllowed}
    />
  );
};
