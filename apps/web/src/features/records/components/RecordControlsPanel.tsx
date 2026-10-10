import { useQuery } from '@tanstack/react-query';
import { Box, Chip, Paper, Stack, Typography } from '@mui/material';
import { useId, type ReactNode } from 'react';
import { monoFontFamily } from '../../../app/theme';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../../components/QueryErrorNotice';
import { Timestamp } from '../../../time/Timestamp';
import type { Attribute, RecordApproval, RetentionHold } from '../api';
import { attributeLabel } from '../recordDisplay';
import {
  recordApprovalsOptions,
  recordRetentionHoldsOptions,
} from '../queryOptions';
import { statusLabel } from '../status';

/** Display labels for attribute and status codes the panel reports. */
const controlLabels = (attributes: readonly Attribute[]) => {
  const byCode = new Map(
    attributes.map((attribute) => [attribute.code, attribute]),
  );
  return {
    attribute: (code: string) => {
      const attribute = byCode.get(code);
      return attribute ? attributeLabel(attribute) : code;
    },
    status: (attributeCode: string | null, status: string) => {
      const attribute = attributeCode ? byCode.get(attributeCode) : undefined;
      return (attribute && statusLabel(attribute, status)) ?? status;
    },
  };
};
type ControlLabels = ReturnType<typeof controlLabels>;

/** One approval or hold: its title and state chip, then its details. */
const ControlItem = ({
  title,
  state,
  children,
}: {
  title: string;
  state: { label: string; color: 'default' | 'success' | 'warning' };
  children: ReactNode;
}) => (
  <Box component="li" sx={{ py: 1.5, borderBottom: 1, borderColor: 'divider' }}>
    <Stack
      direction="row"
      spacing={1}
      sx={{ alignItems: 'baseline', flexWrap: 'wrap' }}
    >
      <Typography variant="subtitle2">{title}</Typography>
      <Chip
        size="small"
        label={state.label}
        color={state.color}
        variant="outlined"
      />
    </Stack>
    {children}
  </Box>
);

const ApprovalItem = ({
  approval,
  labels,
}: {
  approval: RecordApproval;
  labels: ControlLabels;
}) => {
  const { t } = useTranslation();
  const state =
    approval.end_reason === 'content_changed'
      ? t('records.recordControls.approvalVoided')
      : approval.end_reason === 'superseded'
        ? t('records.recordControls.approvalSuperseded')
        : t('records.recordControls.approvalActive');
  return (
    <ControlItem
      title={t('records.recordControls.approvedAs', {
        status: labels.status(approval.attribute_code, approval.status),
        context: approval.context_code,
      })}
      state={{
        label: state,
        color: approval.end_reason === null ? 'success' : 'default',
      }}
    >
      <Typography variant="body2" color="text.secondary">
        {t('records.recordControls.approvedBy', {
          user: approval.approved_by ?? t('audit.system'),
        })}{' '}
        <Timestamp value={approval.approved_at} />
      </Typography>
      {approval.end_reason === 'content_changed' && approval.ended_at && (
        <Typography variant="body2" color="text.secondary">
          {t('records.recordControls.voidedOn')}{' '}
          <Timestamp value={approval.ended_at} />
          {approval.void_status &&
            ` · ${t('records.recordControls.returnedTo', {
              status: labels.status(
                approval.attribute_code,
                approval.void_status,
              ),
            })}`}
        </Typography>
      )}
      <Typography
        variant="caption"
        color="text.secondary"
        sx={{ fontFamily: monoFontFamily, wordBreak: 'break-all' }}
      >
        {t('records.recordControls.digest', {
          digest: approval.content_digest,
        })}
      </Typography>
    </ControlItem>
  );
};

const HoldItem = ({
  hold,
  labels,
}: {
  hold: RetentionHold;
  labels: ControlLabels;
}) => {
  const { t } = useTranslation();
  return (
    <ControlItem
      title={
        hold.source === 'status'
          ? t('records.recordControls.statusHold', {
              attribute: hold.attribute_code
                ? labels.attribute(hold.attribute_code)
                : '',
              status: hold.status
                ? labels.status(hold.attribute_code, hold.status)
                : '',
            })
          : t('records.recordControls.explicitHold', {
              reason: hold.reason ?? '',
            })
      }
      state={
        hold.active
          ? { label: t('records.recordControls.holdActive'), color: 'warning' }
          : { label: t('records.recordControls.holdEnded'), color: 'default' }
      }
    >
      <Typography variant="body2" color="text.secondary">
        {t('records.recordControls.heldUntil')}{' '}
        <Timestamp value={hold.held_until} />
      </Typography>
    </ControlItem>
  );
};

/**
 * Approval history and file retention holds of a controlled record. Renders
 * nothing for records without either. `attributes` label the reported codes.
 */
export const RecordControlsPanel = ({
  recordId,
  attributes,
}: {
  recordId: string;
  attributes: readonly Attribute[];
}) => {
  const { t } = useTranslation();
  const headingId = useId();
  const approvals = useQuery(recordApprovalsOptions(recordId));
  const holds = useQuery(recordRetentionHoldsOptions(recordId));
  const labels = controlLabels(attributes);
  const error = approvals.error ?? holds.error;
  if (!error && !approvals.data?.length && !holds.data?.length) return null;
  return (
    <Paper
      component="section"
      aria-labelledby={headingId}
      sx={{ mt: 3, p: { xs: 2, md: 3 } }}
    >
      <Typography id={headingId} component="h2" variant="h6" sx={{ mb: 1 }}>
        {t('records.recordControls.title')}
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
            {t('records.recordControls.approvals')}
          </Typography>
          <Box component="ul" sx={{ listStyle: 'none', p: 0, m: 0 }}>
            {approvals.data?.map((approval) => (
              <ApprovalItem
                key={approval.id}
                approval={approval}
                labels={labels}
              />
            ))}
          </Box>
        </>
      )}
      {Boolean(holds.data?.length) && (
        <>
          <Typography component="h3" variant="subtitle1" sx={{ mt: 2 }}>
            {t('records.recordControls.retentionHolds')}
          </Typography>
          <Box component="ul" sx={{ listStyle: 'none', p: 0, m: 0 }}>
            {holds.data?.map((hold) => (
              <HoldItem key={hold.id} hold={hold} labels={labels} />
            ))}
          </Box>
        </>
      )}
    </Paper>
  );
};
