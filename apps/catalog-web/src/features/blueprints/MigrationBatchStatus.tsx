import RefreshIcon from '@mui/icons-material/Refresh';
import type { UseQueryResult } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Chip,
  LinearProgress,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { formatBlueprintDateTime } from './date-time';
import type { BlueprintMigrationBatchStatus as MigrationBatch } from './schemas';

const statusColor = (status: MigrationBatch['status']) => {
  if (status === 'completed') return 'success';
  if (status === 'queued' || status === 'running') return 'info';
  if (status === 'superseded') return 'default';
  return 'warning';
};

export const MigrationBatchStatus = ({
  batches,
}: {
  batches: UseQueryResult<MigrationBatch[], Error>;
}) => {
  const { t } = useTranslation();
  const emptyDate = t('blueprints.migrationNotStarted');

  return (
    <Paper component="section" sx={{ mt: 3, p: 2.5 }}>
      <Stack
        direction={{ xs: 'column', sm: 'row' }}
        spacing={1}
        sx={{ alignItems: { sm: 'center' }, justifyContent: 'space-between' }}
      >
        <div>
          <Typography component="h2" variant="h6">
            {t('blueprints.migrationBatches')}
          </Typography>
          <Typography color="text.secondary" sx={{ mt: 0.5 }}>
            {t('blueprints.migrationBatchesDescription')}
          </Typography>
        </div>
        <Button
          disabled={batches.isFetching}
          onClick={() => batches.refetch()}
          startIcon={<RefreshIcon />}
          variant="outlined"
        >
          {batches.isFetching
            ? t('blueprints.refreshingMigrations')
            : t('blueprints.refreshMigrations')}
        </Button>
      </Stack>
      {batches.isError && (
        <Alert severity="error" sx={{ mt: 2 }}>
          {batches.error.message}
        </Alert>
      )}
      {batches.isPending && (
        <Typography sx={{ mt: 2 }}>
          {t('blueprints.loadingMigrations')}
        </Typography>
      )}
      {batches.data?.length === 0 && (
        <Typography color="text.secondary" sx={{ mt: 2 }}>
          {t('blueprints.noMigrationBatches')}
        </Typography>
      )}
      {batches.data && batches.data.length > 0 && (
        <TableContainer sx={{ mt: 2 }}>
          <Table aria-label={t('blueprints.migrationBatches')} size="small">
            <TableHead>
              <TableRow>
                <TableCell>{t('blueprints.targetVersion')}</TableCell>
                <TableCell>{t('blueprints.status')}</TableCell>
                <TableCell>{t('blueprints.migrationProgress')}</TableCell>
                <TableCell>{t('blueprints.migratedEntities')}</TableCell>
                <TableCell>{t('blueprints.needsReviewEntities')}</TableCell>
                <TableCell>{t('blueprints.failedEntities')}</TableCell>
                <TableCell>{t('blueprints.created')}</TableCell>
                <TableCell>{t('blueprints.started')}</TableCell>
                <TableCell>{t('blueprints.completed')}</TableCell>
                <TableCell>{t('blueprints.batchId')}</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {batches.data.map((batch) => (
                <TableRow key={batch.id}>
                  <TableCell>v{batch.target_version}</TableCell>
                  <TableCell>
                    <Chip
                      color={statusColor(batch.status)}
                      label={t(`blueprints.migrationStatuses.${batch.status}`)}
                      size="small"
                    />
                  </TableCell>
                  <TableCell sx={{ minWidth: 150 }}>
                    <Typography variant="body2">
                      {t('blueprints.processedEntities', {
                        processed: batch.processed_entities,
                        total: batch.total_entities,
                      })}
                    </Typography>
                    <LinearProgress
                      aria-label={t('blueprints.migrationProgress')}
                      sx={{ mt: 0.5 }}
                      value={
                        batch.total_entities === 0
                          ? 100
                          : Math.min(
                              100,
                              (batch.processed_entities /
                                batch.total_entities) *
                                100,
                            )
                      }
                      variant="determinate"
                    />
                  </TableCell>
                  <TableCell>{batch.migrated_entities}</TableCell>
                  <TableCell>{batch.needs_input_entities}</TableCell>
                  <TableCell>{batch.failed_entities}</TableCell>
                  <TableCell>
                    {formatBlueprintDateTime(batch.created_at, emptyDate)}
                  </TableCell>
                  <TableCell>
                    {formatBlueprintDateTime(batch.started_at, emptyDate)}
                  </TableCell>
                  <TableCell>
                    {formatBlueprintDateTime(batch.completed_at, emptyDate)}
                  </TableCell>
                  <TableCell sx={{ fontFamily: 'monospace' }}>
                    {batch.id}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      )}
    </Paper>
  );
};
