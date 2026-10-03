import { useQuery } from '@tanstack/react-query';
import { Box, Chip, Paper, Stack, Typography } from '@mui/material';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../../components/QueryErrorNotice';
import { Timestamp } from '../../../time/Timestamp';
import {
  getEntityApprovals,
  getEntityRetentionHolds,
  type EntityApproval,
  type RetentionHold,
} from '../recordControls';
import { entityQueryKeys } from '../queryKeys';

const ApprovalItem = ({ approval }: { approval: EntityApproval }) => {
  const { t } = useTranslation();
  const state =
    approval.end_reason === 'content_changed'
      ? t('entities.recordControls.approvalVoided')
      : approval.end_reason === 'superseded'
        ? t('entities.recordControls.approvalSuperseded')
        : t('entities.recordControls.approvalActive');
  return (
    <Box
      component="li"
      sx={{ py: 1.5, borderBottom: 1, borderColor: 'divider' }}
    >
      <Stack
        direction="row"
        spacing={1}
        sx={{ alignItems: 'baseline', flexWrap: 'wrap' }}
      >
        <Typography variant="subtitle2">
          {t('entities.recordControls.approvedAs', {
            status: approval.status,
            context: approval.context_code,
          })}
        </Typography>
        <Chip
          size="small"
          label={state}
          color={approval.end_reason === null ? 'success' : 'default'}
          variant="outlined"
        />
      </Stack>
      <Typography variant="body2" color="text.secondary">
        {t('entities.recordControls.approvedBy', {
          user: approval.approved_by ?? t('entities.recordControls.system'),
        })}{' '}
        <Timestamp value={approval.approved_at} />
      </Typography>
      {approval.end_reason === 'content_changed' && approval.ended_at && (
        <Typography variant="body2" color="text.secondary">
          {t('entities.recordControls.voidedOn')}{' '}
          <Timestamp value={approval.ended_at} />
          {approval.void_status &&
            ` · ${t('entities.recordControls.returnedTo', {
              status: approval.void_status,
            })}`}
        </Typography>
      )}
      <Typography
        variant="caption"
        color="text.secondary"
        sx={{ fontFamily: 'monospace', wordBreak: 'break-all' }}
      >
        {t('entities.recordControls.digest', {
          digest: approval.content_digest,
        })}
      </Typography>
    </Box>
  );
};

const HoldItem = ({ hold }: { hold: RetentionHold }) => {
  const { t } = useTranslation();
  return (
    <Box
      component="li"
      sx={{ py: 1.5, borderBottom: 1, borderColor: 'divider' }}
    >
      <Stack
        direction="row"
        spacing={1}
        sx={{ alignItems: 'baseline', flexWrap: 'wrap' }}
      >
        <Typography variant="subtitle2">
          {hold.source === 'status'
            ? t('entities.recordControls.statusHold', {
                attribute: hold.attribute_code ?? '',
                status: hold.status ?? '',
              })
            : t('entities.recordControls.explicitHold', {
                reason: hold.reason ?? '',
              })}
        </Typography>
        <Chip
          size="small"
          label={
            hold.active
              ? t('entities.recordControls.holdActive')
              : t('entities.recordControls.holdEnded')
          }
          color={hold.active ? 'warning' : 'default'}
          variant="outlined"
        />
      </Stack>
      <Typography variant="body2" color="text.secondary">
        {t('entities.recordControls.heldUntil')}{' '}
        <Timestamp value={hold.held_until} />
      </Typography>
    </Box>
  );
};

/**
 * Approval history and file retention holds of a controlled record. Renders
 * nothing for records without either.
 */
export const RecordControlsPanel = ({ entityId }: { entityId: string }) => {
  const { t } = useTranslation();
  const headingId = useId();
  const approvals = useQuery({
    queryKey: [...entityQueryKeys.recordControls(entityId), 'approvals'],
    queryFn: ({ signal }) => getEntityApprovals(entityId, signal),
  });
  const holds = useQuery({
    queryKey: [...entityQueryKeys.recordControls(entityId), 'holds'],
    queryFn: ({ signal }) => getEntityRetentionHolds(entityId, signal),
  });
  const error = approvals.error ?? holds.error;
  if (!error && !approvals.data?.length && !holds.data?.length) return null;
  return (
    <Paper
      component="section"
      aria-labelledby={headingId}
      sx={{ mt: 3, p: { xs: 2, md: 3 } }}
    >
      <Typography id={headingId} component="h2" variant="h6" sx={{ mb: 1 }}>
        {t('entities.recordControls.title')}
      </Typography>
      <QueryErrorNotice
        error={error}
        isRetrying={approvals.isFetching || holds.isFetching}
        onRetry={() => {
          void approvals.refetch();
          void holds.refetch();
        }}
      />
      {Boolean(approvals.data?.length) && (
        <>
          <Typography component="h3" variant="subtitle1" sx={{ mt: 1 }}>
            {t('entities.recordControls.approvals')}
          </Typography>
          <Box component="ul" sx={{ listStyle: 'none', p: 0, m: 0 }}>
            {approvals.data?.map((approval) => (
              <ApprovalItem key={approval.id} approval={approval} />
            ))}
          </Box>
        </>
      )}
      {Boolean(holds.data?.length) && (
        <>
          <Typography component="h3" variant="subtitle1" sx={{ mt: 2 }}>
            {t('entities.recordControls.retentionHolds')}
          </Typography>
          <Box component="ul" sx={{ listStyle: 'none', p: 0, m: 0 }}>
            {holds.data?.map((hold) => (
              <HoldItem key={hold.id} hold={hold} />
            ))}
          </Box>
        </>
      )}
    </Paper>
  );
};
