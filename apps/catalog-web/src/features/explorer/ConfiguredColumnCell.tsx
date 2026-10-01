import { Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { Attribute, EntityItem } from '../entities/api';
import type { ExtensionContribution } from '../extensions/api';
import { UrlDisplay } from '../views/components/UrlDisplay';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../views/constants';
import { AttributeValue } from '../views/components/values/AttributeValue';
import {
  explorerExtensionContextVersion,
  relationshipPathSeparator,
  tableImageRendererId,
} from './constants';
import {
  isRelationshipPath,
  usesExtensionRenderer,
  type ExplorerTableColumn,
} from './explorerTableColumns';
import { ExtensionTableCell } from './ExtensionTableCell';
import { ImageTableCell } from './ImageTableCell';
import { explorerTableCellContextSchema } from './schemas';

const formatProjectedValue = (value: unknown) =>
  typeof value === 'object' ? JSON.stringify(value) : String(value);

/**
 * Search projections contain the related target's scalar but not its
 * attribute definition. Keep that rendering deliberately defensive: an absent
 * relation or incompatible value is not an excuse to fetch a row (or to break
 * virtualized rendering).
 */
const RelatedPathValue = ({ value }: { value: unknown }) => {
  const { t } = useTranslation();
  const missing = value === null || value === undefined;
  return (
    <Typography color={missing ? 'text.secondary' : undefined} variant="body2">
      {missing
        ? t('views.notSet')
        : Array.isArray(value)
          ? value.map(formatProjectedValue).join(', ')
          : formatProjectedValue(value)}
    </Typography>
  );
};

type Props = {
  attribute: Attribute;
  column: ExplorerTableColumn;
  entity: EntityItem;
  extension: ExtensionContribution | undefined;
  frameAllowed: boolean;
};

export const ConfiguredColumnCell = ({
  attribute,
  column,
  entity,
  extension,
  frameAllowed,
}: Props) => {
  const [relationship] = column.field.split(relationshipPathSeparator);
  const relatedPath = isRelationshipPath(column.field);
  const { renderer } = column;
  const related = relatedPath ? entity.related?.[relationship]?.[0] : undefined;
  const pathValues = entity.table_values[column.field] ?? [];
  const primaryValue = pathValues.length > 1 ? pathValues : pathValues[0];
  const fallback = relatedPath ? (
    <RelatedPathValue value={primaryValue} />
  ) : (
    <AttributeValue attribute={attribute} compact value={primaryValue} />
  );
  if (
    renderer?.id === VIEW_COMPONENT_IDS.urlDisplay &&
    renderer.version === VIEW_COMPONENT_VERSION
  )
    return <UrlDisplay value={primaryValue} />;
  if (renderer?.id === tableImageRendererId)
    return <ImageTableCell value={primaryValue} />;
  if (!renderer || !usesExtensionRenderer(column)) return fallback;
  const context = explorerTableCellContextSchema.parse({
    context_version: explorerExtensionContextVersion,
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
          relationship_context_code: related.relationship_context_code,
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
      frameAllowed={frameAllowed}
    />
  );
};
