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
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { BlueprintWithAttributes } from './api';
import { BlueprintViewsPreview } from './BlueprintViewsPreview';
import { JsonMetadata } from './BlueprintMetadata';
import { isHiddenByDefault } from '../entities/attribute-visibility';

export const BlueprintVersionMetadata = ({
  blueprint,
}: {
  blueprint: BlueprintWithAttributes;
}) => {
  const { t } = useTranslation();
  const tabId = useId();
  const [tab, setTab] = useState(0);
  const metadataAttributes = blueprint.attributes.filter(
    (attribute) => !isHiddenByDefault(attribute, 'metadata'),
  );

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
          aria-controls={`${tabId}-tabpanel-0`}
          id={`${tabId}-tab-0`}
          label={t('blueprints.attributes', {
            count: metadataAttributes.length,
          })}
        />
        <Tab
          aria-controls={`${tabId}-tabpanel-1`}
          id={`${tabId}-tab-1`}
          label={t('blueprints.views')}
        />
        <Tab
          aria-controls={`${tabId}-tabpanel-2`}
          id={`${tabId}-tab-2`}
          label={t('blueprints.viewDefinition')}
        />
        <Tab
          aria-controls={`${tabId}-tabpanel-3`}
          id={`${tabId}-tab-3`}
          label={t('blueprints.entitySchema')}
        />
        <Tab
          aria-controls={`${tabId}-tabpanel-4`}
          id={`${tabId}-tab-4`}
          label={t('blueprints.includes')}
        />
        <Tab
          aria-controls={`${tabId}-tabpanel-5`}
          id={`${tabId}-tab-5`}
          label={t('blueprints.publicationPolicy')}
        />
      </Tabs>
      <Box
        aria-labelledby={`${tabId}-tab-${tab}`}
        id={`${tabId}-tabpanel-${tab}`}
        role="tabpanel"
        sx={{ mt: 2 }}
      >
        {tab === 4 && (
          <JsonMetadata
            label={t('blueprints.includes')}
            value={blueprint.blueprint.includes}
          />
        )}
        {tab === 5 && (
          <JsonMetadata
            label={t('blueprints.publicationPolicy')}
            value={{
              retain_on_edit_roles: publicationRoles(
                blueprint.blueprint.definition,
              ),
            }}
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
                {metadataAttributes.map((attribute) => (
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

const publicationRoles = (definition: string) => {
  const section = definition
    .split(/^\[publication\]\s*$/m)[1]
    ?.split(/^\[[^\]]+\]\s*$/m)[0];
  const roles = section?.match(/retain_on_edit_roles\s*=\s*\[([^\]]*)\]/)?.[1];
  return roles
    ? [...roles.matchAll(/["']([^"']+)["']/g)].map((match) => match[1])
    : [];
};
