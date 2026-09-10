import {
  Box,
  Paper,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Tabs,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { BlueprintWithAttributes } from './api';
import { BlueprintViewsPreview } from './BlueprintViewsPreview';
import { JsonMetadata } from './BlueprintMetadata';

export const BlueprintVersionMetadata = ({
  blueprint,
}: {
  blueprint: BlueprintWithAttributes;
}) => {
  const { t } = useTranslation();
  const [tab, setTab] = useState(0);

  return (
    <Paper component="section" sx={{ mt: 3, p: 2.5 }}>
      <Typography component="h2" variant="h6">
        {t('blueprints.versionMetadata', {
          version: blueprint.blueprint.version,
        })}
      </Typography>
      <Tabs
        allowScrollButtonsMobile
        onChange={(_, value: number) => setTab(value)}
        scrollButtons="auto"
        sx={{ mt: 1 }}
        value={tab}
        variant="scrollable"
      >
        <Tab
          label={t('blueprints.attributes', {
            count: blueprint.attributes.length,
          })}
        />
        <Tab label={t('blueprints.views')} />
        <Tab label={t('blueprints.viewDefinition')} />
        <Tab label={t('blueprints.entitySchema')} />
        <Tab label={t('blueprints.includes')} />
      </Tabs>
      <Box sx={{ mt: 2 }}>
        {tab === 4 && (
          <JsonMetadata
            label={t('blueprints.includes')}
            value={blueprint.blueprint.includes}
          />
        )}
        {tab === 1 && (
          <BlueprintViewsPreview
            attributes={blueprint.attributes}
            views={blueprint.blueprint.views}
          />
        )}
        {tab === 2 && (
          <JsonMetadata
            label={t('blueprints.views')}
            value={blueprint.blueprint.views}
          />
        )}
        {tab === 3 && (
          <JsonMetadata
            label={t('blueprints.entitySchema')}
            value={blueprint.blueprint.entity_schema}
          />
        )}
        {tab === 0 && (
          <Box sx={{ overflowX: 'auto' }}>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>{t('blueprints.code')}</TableCell>
                  <TableCell>{t('blueprints.type')}</TableCell>
                  <TableCell>{t('blueprints.target')}</TableCell>
                  <TableCell>{t('blueprints.tags')}</TableCell>
                  <TableCell>{t('blueprints.valueSchema')}</TableCell>
                  <TableCell>{t('blueprints.context')}</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {blueprint.attributes.map((attribute) => (
                  <TableRow key={String(attribute.id)}>
                    <TableCell>{attribute.code}</TableCell>
                    <TableCell>{attribute.value_type}</TableCell>
                    <TableCell>
                      {attribute.target_blueprint_code ?? '—'}
                    </TableCell>
                    <TableCell>{JSON.stringify(attribute.tags)}</TableCell>
                    <TableCell>
                      {JSON.stringify(attribute.value_schema)}
                    </TableCell>
                    <TableCell>
                      {attribute.context_fallback} /{' '}
                      {attribute.context_editable}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Box>
        )}
      </Box>
    </Paper>
  );
};
