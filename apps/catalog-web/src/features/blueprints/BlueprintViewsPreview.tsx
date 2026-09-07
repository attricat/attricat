import { createElement, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Box,
  Button,
  MenuItem,
  Paper,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Tabs,
  TextField,
  Typography,
} from '@mui/material';
import type { Attribute, Blueprint, ViewDefinition } from '../entities/api';
import { attributeValueTypes } from '../entities/value-types';
import { EntityView } from '../views/components/EntityView';
import {
  entityHeadingComponentId,
  findEntityHeading,
} from '../views/components/blocks/EntityHeadingDefinition';
import { resolveHeadingRenderer } from '../views/components/registry';
import { AttributeValue } from '../views/components/values/AttributeValue';
import { sandboxValuesForFields } from './sandbox-values';

const inputPlaceholder = (attribute: Attribute) => {
  if (attribute.value_type === attributeValueTypes.date) return 'YYYY-MM-DD';
  if (attribute.value_type === attributeValueTypes.datetime)
    return '2026-08-19T12:00:00Z';
  if (attribute.value_type === attributeValueTypes.time)
    return '14:30:00 America/New_York';
  return undefined;
};

const TableViewPreview = ({
  attributes,
  fields,
  values,
}: {
  attributes: readonly Attribute[];
  fields: readonly string[];
  values: ReturnType<typeof sandboxValuesForFields>;
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
              <TableCell key={field}>{field.replaceAll('_', ' ')}</TableCell>
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

const RenderedView = ({
  attributes,
  values,
  view,
}: {
  attributes: readonly Attribute[];
  values: ReturnType<typeof sandboxValuesForFields>;
  view: ViewDefinition;
}) => {
  if (view.type === 'table' || view.type === 'dropdown_option')
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
          entityId: 'Sandbox entity',
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

export const BlueprintViewsPreview = ({
  attributes,
  views,
}: {
  attributes: readonly Attribute[];
  views: Blueprint['views'];
}) => {
  const { t } = useTranslation();
  const [fields, setFields] = useState<Record<string, string>>({});
  const entries = Object.entries(views);
  const editEntry = entries.find(([name]) => name === 'edit');
  const previewEntries = entries.filter(([name]) => name !== 'edit');
  const tabs: [string, ViewDefinition | undefined][] = editEntry
    ? [editEntry, ...previewEntries]
    : [['edit', undefined], ...previewEntries];
  const [selectedView, setSelectedView] = useState(tabs[0][0]);
  const activeView = tabs.find(([name]) => name === selectedView) ?? tabs[0];
  const values = sandboxValuesForFields(attributes, fields);

  return (
    <>
      <Typography color="text.secondary" sx={{ mt: 1 }}>
        {t('blueprints.sandboxDescription')}
      </Typography>
      <Tabs
        allowScrollButtonsMobile
        onChange={(_, value: string) => setSelectedView(value)}
        scrollButtons="auto"
        sx={{ mt: 1 }}
        value={activeView?.[0] ?? false}
        variant="scrollable"
      >
        {tabs.map(([name]) => (
          <Tab key={name} label={name} value={name} />
        ))}
      </Tabs>
      <Paper variant="outlined" sx={{ mt: 2, p: 2.5 }}>
        {activeView?.[0] === 'edit' ? (
          <EntityView
            attributes={attributes}
            renderEditor={(attribute) => {
              const value = fields[attribute.code] ?? '';
              const update = (next: string) =>
                setFields((current) => ({
                  ...current,
                  [attribute.code]: next,
                }));
              if (attribute.value_type === attributeValueTypes.relationship)
                return (
                  <TextField
                    disabled
                    fullWidth
                    helperText={t('blueprints.relationshipSandboxUnavailable')}
                    label={attribute.code}
                    value=""
                  />
                );

              if (attribute.value_type === attributeValueTypes.boolean) {
                return (
                  <TextField
                    fullWidth
                    label={attribute.code}
                    onChange={(event) => update(event.target.value)}
                    select
                    value={value}
                  >
                    <MenuItem value="">{t('blueprints.notSet')}</MenuItem>
                    <MenuItem value="true">{t('blueprints.true')}</MenuItem>
                    <MenuItem value="false">{t('blueprints.false')}</MenuItem>
                  </TextField>
                );
              }

              if (attribute.value_type === attributeValueTypes.file) {
                return <Button>{t('blueprints.chooseOrDropFiles')}</Button>;
              }

              return (
                <TextField
                  fullWidth
                  label={attribute.code}
                  onChange={(event) => update(event.target.value)}
                  placeholder={inputPlaceholder(attribute)}
                  value={value}
                />
              );
            }}
            values={values}
            view={activeView[1]}
          />
        ) : (
          activeView[1] && (
            <RenderedView
              attributes={attributes}
              values={values}
              view={activeView[1]}
            />
          )
        )}
      </Paper>
    </>
  );
};
