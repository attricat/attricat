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
import type { Attribute, ViewDefinition } from '../entities/api';
import { viewBlockTypes } from '../entities/schemas';
import { EntityView } from '../views/components/EntityView';
import {
  entityHeadingComponentId,
  findEntityHeading,
} from '../views/components/blocks/EntityHeadingDefinition';
import { resolveHeadingRenderer } from '../views/components/registry';
import { AttributeValue } from '../views/components/values/AttributeValue';
import type { SandboxValues } from './sandboxValues';

const fieldLabel = (field: string) => field.replaceAll('_', ' ');

const TableViewPreview = ({
  attributes,
  fields,
  values,
}: {
  attributes: readonly Attribute[];
  fields: readonly string[];
  values: SandboxValues;
}) => {
  const attributesByCode = new Map(
    attributes.map((attribute) => [attribute.code, attribute]),
  );
  const visibleFields = fields.flatMap((field) => {
    const attribute = attributesByCode.get(field);
    return attribute ? [[field, attribute] as const] : [];
  });
  return (
    <Box sx={{ overflowX: 'auto' }}>
      <Table size="small">
        <TableHead>
          <TableRow>
            {visibleFields.map(([field]) => (
              <TableCell key={field}>{fieldLabel(field)}</TableCell>
            ))}
          </TableRow>
        </TableHead>
        <TableBody>
          <TableRow>
            {visibleFields.map(([field, attribute]) => (
              <TableCell key={field}>
                <AttributeValue
                  attribute={attribute}
                  value={values[field]?.value}
                />
              </TableCell>
            ))}
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
        values={values}
      />
    );
  const heading = findEntityHeading(view);
  const HeadingRenderer = resolveHeadingRenderer(heading?.component);
  return (
    <>
      {HeadingRenderer &&
        createElement(HeadingRenderer, {
          attributes,
          entityId: t('blueprints.sandboxEntity'),
          values,
          view,
        })}
      <EntityView
        attributes={attributes}
        skipComponentId={entityHeadingComponentId}
        values={values}
        view={view}
      />
    </>
  );
};
