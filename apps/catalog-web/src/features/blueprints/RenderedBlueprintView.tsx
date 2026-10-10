import {
  Box,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
} from '@mui/material';
import { createElement } from 'react';
import { useTranslation } from 'react-i18next';
import type {
  Attribute,
  ComponentReference,
  ViewDefinition,
} from '../records/api';
import { attributeLabel } from '../records/recordDisplay';
import { viewBlockTypes } from '../records/schemas';
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

const TableViewPreview = ({
  attributes,
  fields,
  columns,
  values,
}: {
  attributes: readonly Attribute[];
  fields: readonly string[];
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
    return attribute ? [{ ...column, attribute }] : [];
  });
  return (
    <Box sx={{ overflowX: 'auto' }}>
      <Table size="small">
        <TableHead>
          <TableRow>
            {visibleFields.map(({ field, label, attribute }) => (
              <TableCell key={field}>
                {label ? lexiconText(label) : attributeLabel(attribute)}
              </TableCell>
            ))}
          </TableRow>
        </TableHead>
        <TableBody>
          <TableRow>
            {visibleFields.map(({ field, attribute, renderer }) => {
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
  values,
  view,
}: {
  attributes: readonly Attribute[];
  values: SandboxValues;
  view: ViewDefinition;
}) => {
  const { t } = useTranslation();
  if (
    view.type === viewBlockTypes.table ||
    view.type === viewBlockTypes.dropdownOption
  )
    return (
      <TableViewPreview
        attributes={attributes}
        fields={view.fields}
        columns={view.type === viewBlockTypes.table ? view.columns : undefined}
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
          recordId: t('blueprints.sandboxRecord'),
          values,
          view,
        })}
      <RecordView
        attributes={attributes}
        skipComponentId={recordHeadingComponentId}
        values={values}
        view={view}
      />
    </>
  );
};
