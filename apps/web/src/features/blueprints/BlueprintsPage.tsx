import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Box,
  Chip,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PencilIcon, PlusIcon } from 'lucide-react';
import { PageContainer } from '../../components/PageContainer';
import { RouterButton } from '../../components/RouterLink';
import { PageHeader } from '../../components/PageHeader';
import { blueprintFilterWidth } from './constants';
import { RevisionStatusChip } from '../../components/RevisionStatusChip';
import { Timestamp } from '../../time/Timestamp';
import { EmptyState } from '../../components/EmptyState';
import { BlueprintIcon } from '../../components/systemIcons';
import { lexiconText } from '../lexicon/lexicon';
import { useAccess, useCan } from '../auth/useCan';
import { capabilities, accessTargetKinds } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { blueprintCatalogueOptions } from './queryOptions';
import { blueprintKindLabel } from './constants';

export const BlueprintsPage = () => {
  const { t } = useTranslation();
  const canEdit = useCan(capabilities.blueprintsWrite);
  const [query, setQuery] = useState('');
  const blueprints = useQuery(blueprintCatalogueOptions());
  const editAccess = useAccess(
    capabilities.blueprintsWrite,
    accessTargetKinds.blueprint,
    (blueprints.data ?? []).map((blueprint) => blueprint.id),
  );
  const matchingBlueprints = (blueprints.data ?? []).filter((blueprint) => {
    const term = query.toLowerCase();
    return (
      blueprint.code.toLowerCase().includes(term) ||
      blueprint.name.toLowerCase().includes(term) ||
      lexiconText(blueprint.name).toLowerCase().includes(term) ||
      blueprint.kind.toLowerCase().includes(term) ||
      blueprintKindLabel(t, blueprint.kind).toLowerCase().includes(term) ||
      blueprint.status.toLowerCase().includes(term)
    );
  });

  return (
    <PageContainer>
      <PageHeader
        actions={
          canEdit && (
            <RouterButton
              startIcon={<PlusIcon />}
              to="/manage/blueprints/new"
              variant="contained"
            >
              {t('blueprints.newBlueprintAction')}
            </RouterButton>
          )
        }
        description={t('blueprints.blueprintsDescription')}
        icon={BlueprintIcon}
        title={t('blueprints.blueprints')}
      />
      <TextField
        label={t('blueprints.filterBlueprints')}
        onChange={(event) => setQuery(event.target.value)}
        placeholder={t('blueprints.filterBlueprintsPlaceholder')}
        sx={{ mt: 3, width: { xs: '100%', sm: blueprintFilterWidth } }}
        value={query}
      />
      {blueprints.isPending && (
        <Typography sx={{ mt: 3 }}>
          {t('blueprints.loadingBlueprints')}
        </Typography>
      )}
      <ApiErrorAlert error={blueprints.error} sx={{ mt: 3 }} />
      {blueprints.data && (
        <Paper component="section" sx={{ mt: 3 }}>
          <Box sx={{ borderBottom: 1, borderColor: 'divider', p: 2 }}>
            <Typography>
              {t('blueprints.blueprintCount', {
                count: matchingBlueprints.length,
              })}
            </Typography>
          </Box>
          <Box sx={{ overflowX: 'auto' }}>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>{t('blueprints.blueprint')}</TableCell>
                  <TableCell>{t('blueprints.kind')}</TableCell>
                  <TableCell>{t('blueprints.latestVersionColumn')}</TableCell>
                  <TableCell>{t('blueprints.status')}</TableCell>
                  <TableCell>{t('blueprints.published')}</TableCell>
                  <TableCell>{t('blueprints.updated')}</TableCell>
                  <TableCell />
                </TableRow>
              </TableHead>
              <TableBody>
                {matchingBlueprints.map((blueprint) => (
                  <TableRow hover key={blueprint.id}>
                    <TableCell>
                      <Stack spacing={0.25}>
                        <Link
                          params={{ blueprintId: blueprint.id }}
                          to="/manage/blueprints/$blueprintId"
                        >
                          {lexiconText(blueprint.name)}
                        </Link>
                        <Typography color="text.secondary" variant="caption">
                          {blueprint.code}
                        </Typography>
                      </Stack>
                    </TableCell>
                    <TableCell>
                      <Chip
                        label={blueprintKindLabel(t, blueprint.kind)}
                        size="small"
                        variant="outlined"
                      />
                    </TableCell>
                    <TableCell>
                      {t('common.versionNumber', {
                        version: blueprint.version,
                      })}
                    </TableCell>
                    <TableCell>
                      <RevisionStatusChip
                        size="small"
                        status={blueprint.status}
                      />
                    </TableCell>
                    <TableCell>
                      <Timestamp
                        fallback={t('blueprints.notPublished')}
                        value={blueprint.published_at}
                      />
                    </TableCell>
                    <TableCell>
                      <Timestamp
                        fallback={t('blueprints.notPublished')}
                        value={blueprint.updated_at}
                      />
                    </TableCell>
                    <TableCell align="right">
                      {editAccess.can(blueprint.id) && (
                        <RouterButton
                          startIcon={<PencilIcon />}
                          params={{
                            blueprintId: blueprint.id,
                            version: String(blueprint.version),
                          }}
                          size="small"
                          to="/manage/blueprints/$blueprintId/revisions/$version/new"
                        >
                          {t('blueprints.edit')}
                        </RouterButton>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Box>
          {matchingBlueprints.length === 0 && (
            <EmptyState
              icon={BlueprintIcon}
              title={t('blueprints.noMatchingBlueprints')}
            />
          )}
        </Paper>
      )}
    </PageContainer>
  );
};
