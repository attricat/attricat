import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
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
import { PageContainer } from '../../components/PageContainer';
import { RouterButton } from '../../components/RouterLink';
import { PageHeader } from '../../components/PageHeader';
import { listBlueprints } from './api';
import { blueprintFilterWidth, blueprintStatusChipColor } from './constants';
import { Timestamp } from '../../time/Timestamp';
import { blueprintQueryKeys } from './queryKeys';

export const BlueprintsPage = () => {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const blueprints = useQuery({
    queryKey: blueprintQueryKeys.catalogue(),
    queryFn: listBlueprints,
  });
  const matchingBlueprints = (blueprints.data ?? []).filter((blueprint) => {
    const term = query.toLowerCase();
    return (
      blueprint.code.toLowerCase().includes(term) ||
      blueprint.name.toLowerCase().includes(term) ||
      blueprint.kind.toLowerCase().includes(term) ||
      blueprint.status.toLowerCase().includes(term)
    );
  });

  return (
    <PageContainer>
      <PageHeader
        actions={
          <Button
            component={Link}
            to="/manage/blueprints/new"
            variant="contained"
          >
            {t('blueprints.newBlueprintAction')}
          </Button>
        }
        description={t('blueprints.blueprintsDescription')}
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
      {blueprints.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {blueprints.error.message}
        </Alert>
      )}
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
                          {blueprint.name}
                        </Link>
                        <Typography color="text.secondary" variant="caption">
                          {blueprint.code}
                        </Typography>
                      </Stack>
                    </TableCell>
                    <TableCell>
                      <Chip
                        label={blueprint.kind}
                        size="small"
                        variant="outlined"
                      />
                    </TableCell>
                    <TableCell>
                      {t('blueprints.versionNumber', {
                        version: blueprint.version,
                      })}
                    </TableCell>
                    <TableCell>
                      <Chip
                        color={blueprintStatusChipColor(blueprint.status)}
                        label={t(
                          `blueprints.revisionStatuses.${blueprint.status}`,
                        )}
                        size="small"
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
                      <RouterButton
                        params={{
                          blueprintId: blueprint.id,
                          version: String(blueprint.version),
                        }}
                        size="small"
                        to="/manage/blueprints/$blueprintId/revisions/$version/new"
                      >
                        {t('blueprints.edit')}
                      </RouterButton>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Box>
          {matchingBlueprints.length === 0 && (
            <Typography sx={{ p: 2 }}>
              {t('blueprints.noMatchingBlueprints')}
            </Typography>
          )}
        </Paper>
      )}
    </PageContainer>
  );
};
