import { defaultContextCode } from '../../contexts/constants';
import { LoadMoreButton } from '../../../components/LoadMoreButton';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Box, Chip, CircularProgress, Stack, Typography } from '@mui/material';
import { useId, type ReactNode } from 'react';
import { monoFontFamily } from '../../../app/theme';
import { Trans, useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../../components/QueryErrorNotice';
import { UserProfileLink } from '../../../components/UserProfileLink';
import { Timestamp } from '../../../time/Timestamp';
import type { ApprovalCause, Attribute, RetentionHold } from '../api';
import { attributeLabel } from '../recordDisplay';
import {
  recordApprovalsOptions,
  recordRetentionHoldsOptions,
} from '../queryOptions';
import { statusLabel } from '../status';
import { groupApprovals, type ApprovalGroup } from '../approvalGroups';
import {
  retentionHoldSources,
  approvalEndReasons,
  approvalCauseKinds,
  transitionChannels,
  transitionOperations,
} from '../constants';

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

/** The status change that entered an approval's status, then how it arrived. */
const ApprovalCauseText = ({
  attributeCode,
  cause,
  labels,
}: {
  attributeCode: string;
  cause: ApprovalCause | null;
  labels: ControlLabels;
}) => {
  const { t } = useTranslation();
  if (!cause) {
    return (
      <Typography variant="body2" color="text.secondary">
        {t('records.recordControls.causeUnrecorded')}
      </Typography>
    );
  }
  const from = cause.from && labels.status(attributeCode, cause.from);
  const transition = cause.transition_code;
  const entered =
    cause.kind === approvalCauseKinds.override
      ? t('records.recordControls.causeOverride', { from })
      : from
        ? transition
          ? t('records.recordControls.causeFrom', { from, transition })
          : t('records.recordControls.causeFromWithoutCode', { from })
        : transition
          ? t('records.recordControls.causeInitial', { transition })
          : t('records.recordControls.causeInitialWithoutCode');
  const operation =
    cause.operation === transitionOperations.historyRestore
      ? t('records.recordControls.causeHistoryRestore')
      : cause.operation === transitionOperations.blueprintMigration
        ? t('records.recordControls.causeBlueprintMigration')
        : null;
  const channels = {
    [transitionChannels.person]: null,
    [transitionChannels.apiToken]: t('records.recordControls.causeApiToken'),
    [transitionChannels.agent]: t('records.recordControls.causeAgent'),
    [transitionChannels.workflow]: t('records.recordControls.causeWorkflow'),
    [transitionChannels.extension]: t('records.recordControls.causeExtension'),
    [transitionChannels.eventHandler]: t(
      'records.recordControls.causeEventHandler',
    ),
    [transitionChannels.system]: t('records.recordControls.causeSystem'),
  };
  const channel = cause.channel ? channels[cause.channel] : null;
  return (
    <Typography variant="body2" color="text.secondary">
      {[
        entered,
        operation,
        channel,
        cause.separation_waived &&
          t('records.recordControls.causeSeparationWaived'),
      ]
        .filter(Boolean)
        .join(' · ')}
    </Typography>
  );
};

const ApprovalItem = ({
  group: { approval, contexts },
  labels,
}: {
  group: ApprovalGroup;
  labels: ControlLabels;
}) => {
  const { t } = useTranslation();
  const status = labels.status(approval.attribute_code, approval.status);
  const digests = new Set(contexts.map((context) => context.digest));
  const state =
    approval.end_reason === approvalEndReasons.contentChanged
      ? t('records.recordControls.approvalVoided')
      : approval.end_reason === approvalEndReasons.superseded
        ? t('records.recordControls.approvalSuperseded')
        : t('records.recordControls.approvalActive');
  return (
    <ControlItem
      title={
        contexts.length > 1
          ? t('records.recordControls.approvedAsInContexts', {
              status,
              count: contexts.length,
            })
          : approval.context_code === defaultContextCode
            ? t('records.recordControls.approvedAsInDefaultContext', {
                status,
              })
            : t('records.recordControls.approvedAs', {
                status,
                context: approval.context_code,
              })
      }
      state={{
        label: state,
        color: approval.end_reason === null ? 'success' : 'default',
      }}
    >
      {contexts.length > 1 && (
        <Typography variant="body2" color="text.secondary">
          {t('records.recordControls.contexts', {
            contexts: contexts.map((context) => context.code).join(', '),
          })}
        </Typography>
      )}
      <Typography variant="body2" color="text.secondary">
        <Trans
          components={{
            user: approval.approved_by_user_id ? (
              <UserProfileLink userId={approval.approved_by_user_id} />
            ) : (
              <span />
            ),
          }}
          i18nKey="records.recordControls.approvedBy"
          t={t}
          values={{ user: approval.approved_by ?? t('audit.system') }}
        />{' '}
        <Timestamp value={approval.approved_at} />
      </Typography>
      <ApprovalCauseText
        attributeCode={approval.attribute_code}
        cause={approval.cause}
        labels={labels}
      />
      {approval.end_reason === approvalEndReasons.contentChanged &&
        approval.ended_at && (
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
      {digests.size === 1 ? (
        <Typography
          variant="caption"
          color="text.secondary"
          sx={{ fontFamily: monoFontFamily, wordBreak: 'break-all' }}
        >
          {t('records.recordControls.digest', {
            digest: approval.content_digest,
          })}
        </Typography>
      ) : (
        contexts.map((context) => (
          <Typography
            key={context.id}
            variant="caption"
            component="p"
            color="text.secondary"
            sx={{ fontFamily: monoFontFamily, wordBreak: 'break-all' }}
          >
            {t('records.recordControls.contextDigest', {
              context: context.code,
              digest: context.digest,
            })}
          </Typography>
        ))
      )}
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
        hold.source === retentionHoldSources.status
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
 * Approval history and file retention holds of a controlled record, page by
 * page. `attributes` label the reported codes. Shown only while
 * `useHasRecordControls` holds.
 */
export const RecordControlsPanel = ({
  recordId,
  attributes,
}: {
  recordId: string;
  attributes: readonly Attribute[];
}) => {
  const { t } = useTranslation();
  const approvalsHeadingId = useId();
  const holdsHeadingId = useId();
  const approvals = useInfiniteQuery(recordApprovalsOptions(recordId));
  const holds = useInfiniteQuery(recordRetentionHoldsOptions(recordId));
  const labels = controlLabels(attributes);
  const error = approvals.error ?? holds.error;
  const approvalItems =
    approvals.data?.pages.flatMap((page) => page.items) ?? [];
  const holdItems = holds.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <>
      <QueryErrorNotice
        error={error}
        isRetrying={approvals.isFetching || holds.isFetching}
        onRetry={() => {
          void approvals.refetch();
          void holds.refetch();
        }}
      />
      {(approvals.isPending || holds.isPending) && (
        <CircularProgress
          aria-label={t('records.recordControls.loading')}
          enableTrackSlot
        />
      )}
      {approvalItems.length > 0 && (
        <Box component="section" aria-labelledby={approvalsHeadingId}>
          <Typography
            id={approvalsHeadingId}
            component="h2"
            variant="subtitle1"
          >
            {t('records.recordControls.approvals')}
          </Typography>
          <Box component="ul" sx={{ listStyle: 'none', p: 0, m: 0 }}>
            {groupApprovals(approvalItems).map((group) => (
              <ApprovalItem
                key={group.approval.id}
                group={group}
                labels={labels}
              />
            ))}
          </Box>
          {approvals.hasNextPage && (
            <LoadMoreButton
              compact
              isLoading={approvals.isFetchingNextPage}
              label={t('records.recordControls.loadMoreApprovals')}
              onLoadMore={() => void approvals.fetchNextPage()}
              sx={{ mt: 1 }}
            />
          )}
        </Box>
      )}
      {holdItems.length > 0 && (
        <Box
          component="section"
          aria-labelledby={holdsHeadingId}
          sx={{ mt: approvalItems.length > 0 ? 3 : 0 }}
        >
          <Typography id={holdsHeadingId} component="h2" variant="subtitle1">
            {t('records.recordControls.retentionHolds')}
          </Typography>
          <Box component="ul" sx={{ listStyle: 'none', p: 0, m: 0 }}>
            {holdItems.map((hold) => (
              <HoldItem key={hold.id} hold={hold} labels={labels} />
            ))}
          </Box>
          {holds.hasNextPage && (
            <LoadMoreButton
              compact
              isLoading={holds.isFetchingNextPage}
              label={t('records.recordControls.loadMoreHolds')}
              onLoadMore={() => void holds.fetchNextPage()}
              sx={{ mt: 1 }}
            />
          )}
        </Box>
      )}
    </>
  );
};
