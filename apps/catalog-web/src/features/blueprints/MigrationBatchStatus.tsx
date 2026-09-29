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
import { RefreshCwIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  completePercentage,
  emptyValuePlaceholder,
  migrationBatchStatuses,
  migrationProgressMinWidth,
} from './constants';
import { formatBlueprintDateTime } from './dateTime';
import type { BlueprintMigrationBatchStatus as MigrationBatch } from './schemas';

const statusColor = (status: MigrationBatch['status']) => {
  if (status === migrationBatchStatuses.completed) return 'success';
  if (
    status === migrationBatchStatuses.queued ||
    status === migrationBatchStatuses.running
  )
    return 'info';
  if (status === migrationBatchStatuses.superseded) return 'default';
  return 'warning';
};

const removalPolicyLabel = (
  policy: MigrationBatch['removal_policy'] | undefined,
) => {
  const codes = policy?.attribute_codes;
  return Array.isArray(codes) && codes.every((code) => typeof code === 'string')
    ? codes.join(', ')
    : emptyValuePlaceholder;
};

const progressPercentage = ({
  processed_entities: processed,
  total_entities: total,
}: MigrationBatch) =>
  total === 0
    ? completePercentage
    : Math.min(completePercentage, (processed / total) * completePercentage);

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
          startIcon={<RefreshCwIcon />}
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
                <TableCell>{t('blueprints.removedAttributes')}</TableCell>
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
                  <TableCell>
                    {t('blueprints.versionNumber', {
                      version: batch.target_version,
                    })}
                  </TableCell>
                  <TableCell>
                    <Chip
                      color={statusColor(batch.status)}
                      label={t(`blueprints.migrationStatuses.${batch.status}`)}
                      size="small"
                    />
                  </TableCell>
                  <TableCell sx={{ minWidth: migrationProgressMinWidth }}>
                    <Typography variant="body2">
                      {t('blueprints.processedEntities', {
                        processed: batch.processed_entities,
                        total: batch.total_entities,
                      })}
                    </Typography>
                    <LinearProgress
                      aria-label={t('blueprints.migrationProgress')}
                      sx={{ mt: 0.5 }}
                      value={progressPercentage(batch)}
                      variant="determinate"
                    />
                  </TableCell>
                  <TableCell>{batch.migrated_entities}</TableCell>
                  <TableCell>{batch.needs_input_entities}</TableCell>
                  <TableCell>
                    {removalPolicyLabel(batch.removal_policy)}
                  </TableCell>
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
