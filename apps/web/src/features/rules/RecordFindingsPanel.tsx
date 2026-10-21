import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  Stack,
  Typography,
} from '@mui/material';
import { PlayIcon } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/CheckViolationsAlert';
import { EmptyState } from '../../components/EmptyState';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RuleIcon } from '../../components/systemIcons';
import { Timestamp } from '../../time/Timestamp';
import { useCan } from '../auth/useCan';
import {
  acknowledgeFinding,
  recheckRecord,
  type Finding,
  type RuleRun,
} from './api';
import {
  ACTIVE_RULE_RUN_STATUSES,
  ERROR_FINDING_SEVERITIES,
  FINDING_STATE_OPEN,
  RECORD_RULE_RUN_POLL_MS,
} from './constants';
import { ruleQueryKeys } from './queryKeys';
import {
  recordFindingsOptions,
  recordRuleRunsOptions,
  ruleDefinitionsOptions,
} from './queryOptions';
import { isWatchingRecordRuleChecks } from './recordRuleChecks';
import { RuleRevisionCell } from './RuleCells';
import { ruleRevisionsByKey, type RuleRevisions } from './ruleRevisions';
import { findingEvidenceSchema } from './schemas';

const isActive = (run: RuleRun) =>
  ACTIVE_RULE_RUN_STATUSES.includes(run.status);

/** The latest finish among the runs, which changes whenever a run finishes. */
const latestCompletion = (runs: readonly RuleRun[] | undefined) =>
  runs?.reduce<string | null>(
    (latest, run) =>
      run.completed_at && (!latest || run.completed_at > latest)
        ? run.completed_at
        : latest,
    null,
  ) ?? null;

const FindingItem = ({
  acknowledging,
  canManage,
  finding,
  onAcknowledge,
  revisions,
}: {
  acknowledging: boolean;
  canManage: boolean;
  finding: Finding;
  onAcknowledge: () => void;
  revisions: RuleRevisions;
}) => {
  const { t } = useTranslation();
  const evidence = findingEvidenceSchema.safeParse(finding.evidence);
  const contexts = evidence.success ? (evidence.data.contexts ?? []) : [];
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
        <Chip
          color={
            ERROR_FINDING_SEVERITIES.includes(finding.severity)
              ? 'error'
              : 'warning'
          }
          label={t(`rules.severities.${finding.severity}`, {
            defaultValue: finding.severity,
          })}
          size="small"
        />
        <Typography component="p" variant="subtitle2" sx={{ flex: 1 }}>
          {finding.message}
        </Typography>
        {finding.state === FINDING_STATE_OPEN ? (
          canManage && (
            <Button
              disabled={acknowledging}
              size="small"
              onClick={onAcknowledge}
            >
              {t('rules.acknowledge')}
            </Button>
          )
        ) : (
          <Chip
            label={t(`rules.findingStates.${finding.state}`, {
              defaultValue: finding.state,
            })}
            size="small"
            variant="outlined"
          />
        )}
      </Stack>
      <Typography variant="body2" color="text.secondary">
        <RuleRevisionCell
          revisions={revisions}
          ruleId={finding.rule_id}
          version={finding.rule_version}
        />
        {contexts.length > 0 &&
          ` · ${t('rules.recordFindings.contexts', {
            contexts: contexts.join(', '),
          })}`}
        {' · '}
        <Trans
          i18nKey="rules.recordFindings.firstFound"
          components={{ timestamp: <Timestamp value={finding.created_at} /> }}
        />
      </Typography>
    </Box>
  );
};

/**
 * A record's unresolved data quality findings, page by page, with a re-check
 * of every enabled rule of its blueprint revision. Edits re-check the record
 * on the server; the panel follows those runs and refreshes the findings when
 * they finish.
 */
export const RecordFindingsPanel = ({
  recordId,
  blueprintId,
  blueprintVersion,
}: {
  recordId: string;
  blueprintId: string;
  blueprintVersion: number;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const canRead = useCan('rules_read');
  const canManage = useCan('rules_manage');
  const rules = useQuery({ ...ruleDefinitionsOptions(), enabled: canRead });
  const findings = useInfiniteQuery({
    ...recordFindingsOptions(recordId),
    enabled: canRead,
  });
  const runs = useQuery({
    ...recordRuleRunsOptions(recordId),
    enabled: canRead,
    // Keep checking while runs are pending or an edit's runs may still start.
    refetchInterval: (query) =>
      query.state.data?.some(isActive) || isWatchingRecordRuleChecks(recordId)
        ? RECORD_RULE_RUN_POLL_MS
        : false,
  });
  const finishedAt = latestCompletion(runs.data);
  const seenFinish = useRef<string | null | undefined>(undefined);
  useEffect(() => {
    if (!runs.isSuccess) return;
    if (seenFinish.current !== undefined && seenFinish.current !== finishedAt)
      void client.invalidateQueries({
        queryKey: ruleQueryKeys.findings(recordId),
      });
    seenFinish.current = finishedAt;
  }, [client, finishedAt, recordId, runs.isSuccess]);
  const recheck = useMutation({
    mutationFn: () => recheckRecord(recordId),
    onSuccess: () =>
      client.invalidateQueries({
        queryKey: ruleQueryKeys.recordRuns(recordId),
      }),
  });
  const acknowledge = useMutation({
    mutationFn: acknowledgeFinding,
    onSuccess: () =>
      client.invalidateQueries({ queryKey: ruleQueryKeys.findings() }),
  });

  if (!canRead) return null;
  const hasApplicableRules = (rules.data ?? []).some(
    (rule) =>
      rule.blueprint_id === blueprintId &&
      rule.blueprint_version === blueprintVersion &&
      rule.enabled_version === rule.version,
  );
  const unresolved = findings.data?.pages.flatMap((page) => page.items) ?? [];
  const error = findings.error ?? rules.error ?? runs.error;
  const checking = Boolean(runs.data?.some(isActive)) || recheck.isPending;
  const revisions = ruleRevisionsByKey(rules.data ?? []);

  return (
    <Box aria-busy={checking}>
      {(checking || (canManage && hasApplicableRules)) && (
        <Stack
          direction="row"
          spacing={2}
          sx={{
            alignItems: 'center',
            flexWrap: 'wrap',
            justifyContent: 'flex-end',
            mb: 1,
          }}
        >
          {checking && (
            <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
              <CircularProgress aria-hidden enableTrackSlot size={16} />
              <Typography variant="body2" color="text.secondary" role="status">
                {t('rules.recordFindings.checking')}
              </Typography>
            </Stack>
          )}
          {canManage && hasApplicableRules && (
            <Button
              disabled={checking}
              size="small"
              startIcon={<PlayIcon aria-hidden />}
              onClick={() => recheck.mutate()}
            >
              {t('rules.recordFindings.recheck')}
            </Button>
          )}
        </Stack>
      )}
      <QueryErrorNotice
        error={error}
        isRetrying={findings.isFetching || rules.isFetching || runs.isFetching}
        onRetry={() => {
          void findings.refetch();
          void rules.refetch();
          void runs.refetch();
        }}
      />
      {recheck.error && <ApiErrorAlert error={recheck.error} sx={{ mb: 1 }} />}
      {acknowledge.error && (
        <ApiErrorAlert error={acknowledge.error} sx={{ mb: 1 }} />
      )}
      {findings.isPending && (
        <CircularProgress
          aria-label={t('rules.recordFindings.loading')}
          enableTrackSlot
        />
      )}
      {unresolved.length > 0 ? (
        <Box component="ul" sx={{ listStyle: 'none', p: 0, m: 0 }}>
          {unresolved.map((finding) => (
            <FindingItem
              key={finding.id}
              acknowledging={acknowledge.isPending}
              canManage={canManage}
              finding={finding}
              onAcknowledge={() => acknowledge.mutate(finding.id)}
              revisions={revisions}
            />
          ))}
        </Box>
      ) : (
        findings.isSuccess && (
          <EmptyState
            icon={RuleIcon}
            title={
              finishedAt ? (
                <Trans
                  i18nKey="rules.recordFindings.noFindingsCheckedAt"
                  components={{ timestamp: <Timestamp value={finishedAt} /> }}
                />
              ) : (
                t('rules.recordFindings.noFindings')
              )
            }
            description={
              rules.isSuccess && !hasApplicableRules
                ? t('rules.recordFindings.noRules')
                : undefined
            }
          />
        )
      )}
      {findings.hasNextPage && (
        <Button
          disabled={findings.isFetchingNextPage}
          onClick={() => void findings.fetchNextPage()}
          sx={{ mt: 1 }}
        >
          {t('rules.recordFindings.loadMore')}
        </Button>
      )}
    </Box>
  );
};
