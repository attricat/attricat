import {
  Box,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { createElement } from 'react';
import { useTranslation } from 'react-i18next';
import type {
  Attribute,
  ComponentReference,
  ViewDefinition,
} from '../records/api';
import { isRelationshipValue } from '../records/linkedRecord';
import { resolvedRecordPreviewOptions } from '../records/queryOptions';
import { attributeLabel } from '../records/recordDisplay';
import { blueprintRevisionOptions } from './queryOptions';
import { relationshipPathSeparator } from '../explorer/constants';
import { configuredColumnLabel } from '../explorer/explorerTableColumns';
import { viewBlockTypes } from '../views/schemas';
import { RecordView } from '../views/components/RecordView';
import {
  recordHeadingComponentId,
  findRecordHeading,
} from '../views/components/blocks/RecordHeadingDefinition';
import {
  resolveHeadingRenderer,
  resolveValueRenderer,
} from '../views/components/registry';
import { AttributeValue } from '../views/components/values/AttributeValue';
import type { SandboxValues } from './sandboxValues';
import { lexiconText } from '../lexicon/lexicon';

/**
 * A column that follows a relationship, such as `item.name`, read from the
 * record the chosen record links to. Only one hop is followed; a deeper path
 * stays empty in the preview.
 */
const PathCell = ({
  attributes,
  contextId,
  field,
  values,
}: {
  attributes: readonly Attribute[];
  contextId: string;
  field: string;
  values: SandboxValues;
}) => {
  const [relationship, leaf, ...deeper] = field.split(
    relationshipPathSeparator,
  );
  const link = attributes.find((attribute) => attribute.code === relationship);
  const linked = values[relationship]?.value;
  const targetId =
    deeper.length === 0 && link && isRelationshipValue(linked)
      ? linked.items?.[0]?.id
      : undefined;
  const target = useQuery(
    resolvedRecordPreviewOptions(targetId ?? '', targetId ? contextId : null),
  );
  const revision = useQuery(
    blueprintRevisionOptions(
      target.data?.record.blueprint_id,
      target.data?.record.blueprint_version,
    ),
  );
  const attribute = revision.data?.attributes.find(
    (candidate) => candidate.code === leaf,
  );
  return attribute ? (
    <AttributeValue
      attribute={attribute}
      compact
      value={target.data?.values[leaf]?.value}
    />
  ) : null;
};

const TableViewPreview = ({
  attributes,
  contextId,
  fields,
  columns,
  recordId,
  values,
}: {
  attributes: readonly Attribute[];
  contextId?: string;
  fields: readonly string[];
  /** A chosen record, whose links let path columns show values. */
  recordId?: string;
  columns?: readonly {
    field: string;
    label?: string | null;
    renderer?: ComponentReference | null;
  }[];
  values: SandboxValues;
}) => {
  const attributesByCode = new Map(
    attributes.map((attribute) => [attribute.code, attribute]),
  );
  const visibleFields = (
    columns?.length
      ? columns
      : fields.map((field) => ({
          field,
          label: undefined,
          renderer: undefined,
        }))
  ).flatMap((column) => {
    const attribute = attributesByCode.get(column.field);
    if (attribute) return [{ ...column, attribute, path: false }];
    // A path column follows a link only a chosen record has.
    const link = attributesByCode.get(
      column.field.split(relationshipPathSeparator)[0],
    );
    return recordId &&
      contextId &&
      link &&
      column.field.includes(relationshipPathSeparator)
      ? [{ ...column, attribute: link, path: true }]
      : [];
  });
  return (
    <Box sx={{ overflowX: 'auto' }}>
      <Table size="small">
        <TableHead>
          <TableRow>
            {visibleFields.map(({ field, label, attribute, path }) => (
              <TableCell key={field}>
                {label
                  ? lexiconText(label)
                  : path
                    ? // Labelled like the Explorer's path columns.
                      configuredColumnLabel({ field })
                    : attributeLabel(attribute)}
              </TableCell>
            ))}
          </TableRow>
        </TableHead>
        <TableBody>
          <TableRow>
            {visibleFields.map(({ field, attribute, renderer, path }) => {
              if (path && contextId)
                return (
                  <TableCell key={field}>
                    <PathCell
                      attributes={attributes}
                      contextId={contextId}
                      field={field}
                      values={values}
                    />
                  </TableCell>
                );
              const Renderer = resolveValueRenderer(renderer) ?? AttributeValue;
              return (
                <TableCell key={field}>
                  <Renderer
                    attribute={attribute}
                    component={renderer}
                    value={values[field]?.value}
                  />
                </TableCell>
              );
            })}
          </TableRow>
        </TableBody>
      </Table>
    </Box>
  );
};

/** Read-only preview of a non-form blueprint view using sandbox values. */
export const RenderedBlueprintView = ({
  attributes,
  contextId,
  recordId,
  values,
  view,
}: {
  attributes: readonly Attribute[];
  contextId?: string;
  /** A chosen existing record, whose links and incoming lists render. */
  recordId?: string;
  values: SandboxValues;
  view: ViewDefinition;
}) => {
  const { t } = useTranslation();
  if (
    view.type === viewBlockTypes.table ||
    view.type === viewBlockTypes.dropdownOption ||
    view.type === viewBlockTypes.pageTitle
  )
    return (
      <TableViewPreview
        attributes={attributes}
        contextId={contextId}
        fields={view.type === viewBlockTypes.table ? [] : view.fields}
        columns={view.type === viewBlockTypes.table ? view.columns : undefined}
        recordId={recordId}
        values={values}
      />
    );
  const heading = findRecordHeading(view);
  const HeadingRenderer = resolveHeadingRenderer(heading?.component);
  return (
    <>
      {HeadingRenderer &&
        createElement(HeadingRenderer, {
          attributes,
          recordId: recordId ?? t('blueprints.sandboxRecord'),
          values,
          view,
        })}
      <RecordView
        attributes={attributes}
        contextId={contextId}
        recordId={recordId}
        skipComponentId={recordHeadingComponentId}
        values={values}
        view={view}
      />
    </>
  );
};
