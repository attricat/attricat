import { createElement } from 'react';
import { Typography } from '@mui/material';
import { JsonInline } from '../../components/JsonValue';
import type { Attribute, RecordItem } from '../records/api';
import type { ExtensionContribution } from '../extensions/api';
import { resolveValueRenderer } from '../views/components/registry';
import { AttributeValue } from '../views/components/values/AttributeValue';
import { NotSetValue } from '../views/components/values/NotSetValue';
import { DeclaredCell } from './DeclaredCell';
import {
  isRelationshipPath,
  type ExplorerTableColumn,
} from './explorerTableColumns';

/**
 * Search projections contain the related target's scalar but not its
 * attribute definition. Keep that rendering deliberately defensive: an absent
 * relation or incompatible value is not an excuse to fetch a row (or to break
 * virtualized rendering).
 */
const RelatedPathValue = ({ value }: { value: unknown }) => {
  if (value === null || value === undefined) return <NotSetValue />;
  const values = Array.isArray(value) ? value : [value];
  // Objects are JSON values; render them as JSON rather than plain text.
  if (values.some((item) => typeof item === 'object' && item !== null))
    return <JsonInline noWrap value={value} />;
  return (
    <Typography variant="body2">{values.map(String).join(', ')}</Typography>
  );
};

type Props = {
  attribute: Attribute;
  column: ExplorerTableColumn;
  record: RecordItem;
  /** The enabled extension cell the column's renderer names. */
  extension: ExtensionContribution | undefined;
};

export const ConfiguredColumnCell = ({
  attribute,
  column,
  record,
  extension,
}: Props) => {
  const relatedPath = isRelationshipPath(column.field);
  const { renderer } = column;
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
  // An extension's renderer that is unavailable shows the value as usual.
  if (!renderer || !extension?.cell) return fallback;
  return (
    <DeclaredCell
      cell={extension.cell}
      fallback={fallback}
      props={renderer.props}
      value={primaryValue}
    />
  );
};
