import type { UseQueryResult } from '@tanstack/react-query';
import {
  Button,
  Chip,
  LinearProgress,
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
import { migrationBatchStatuses, migrationProgressMinWidth } from './constants';
import { Timestamp } from '../../time/Timestamp';
import type { BlueprintMigrationBatchStatus as MigrationBatch } from './schemas';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { emptyValuePlaceholder } from '../../components/placeholders';
import { monoFontFamily } from '../../app/theme';
import { LoadingState } from '../../components/LoadingState';
import { EmptyState } from '../../components/EmptyState';
import { MigrationIcon } from '../../components/systemIcons';
import { SectionPaper } from '../../components/SectionPaper';
import { percentOf } from '../../components/percentOf';

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

export const MigrationBatchStatus = ({
  batches,
}: {
  batches: UseQueryResult<MigrationBatch[], Error>;
}) => {
  const { t } = useTranslation();
  const emptyDate = t('blueprints.migrationNotStarted');

  return (
    <SectionPaper>
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
      <ApiErrorAlert error={batches.error} sx={{ mt: 2 }} />
      {batches.isPending && (
        <LoadingState sx={{ mt: 2 }}>
          {t('blueprints.loadingMigrations')}
        </LoadingState>
      )}
      {batches.data?.length === 0 && (
        <EmptyState
          icon={MigrationIcon}
          title={t('blueprints.noMigrationBatches')}
        />
      )}
      {batches.data && batches.data.length > 0 && (
        <TableContainer sx={{ mt: 2 }}>
          <Table aria-label={t('blueprints.migrationBatches')} size="small">
            <TableHead>
              <TableRow>
                <TableCell>{t('blueprints.targetVersion')}</TableCell>
                <TableCell>{t('blueprints.status')}</TableCell>
                <TableCell>{t('blueprints.migrationProgress')}</TableCell>
                <TableCell>{t('blueprints.migratedRecords')}</TableCell>
                <TableCell>{t('blueprints.needsReviewRecords')}</TableCell>
                <TableCell>{t('blueprints.removedAttributes')}</TableCell>
                <TableCell>{t('blueprints.failedRecords')}</TableCell>
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
                    {t('common.versionNumber', {
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
                      {t('blueprints.processedRecords', {
                        processed: batch.processed_records,
                        total: batch.total_records,
                      })}
                    </Typography>
                    <LinearProgress
                      aria-label={t('blueprints.migrationProgress')}
                      sx={{ mt: 0.5 }}
                      value={percentOf(
                        batch.processed_records,
                        batch.total_records,
                      )}
                      variant="determinate"
                    />
                  </TableCell>
                  <TableCell>{batch.migrated_records}</TableCell>
                  <TableCell>{batch.needs_input_records}</TableCell>
                  <TableCell>
                    {removalPolicyLabel(batch.removal_policy)}
                  </TableCell>
                  <TableCell>{batch.failed_records}</TableCell>
                  <TableCell>
                    <Timestamp fallback={emptyDate} value={batch.created_at} />
                  </TableCell>
                  <TableCell>
                    <Timestamp fallback={emptyDate} value={batch.started_at} />
                  </TableCell>
                  <TableCell>
                    <Timestamp
                      fallback={emptyDate}
                      value={batch.completed_at}
                    />
                  </TableCell>
                  <TableCell sx={{ fontFamily: monoFontFamily }}>
                    {batch.id}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      )}
    </SectionPaper>
  );
};
