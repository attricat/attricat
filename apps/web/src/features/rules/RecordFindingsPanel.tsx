import { useAcknowledgeFinding } from './useAcknowledgeFinding';
import { refreshRecordRuleState } from './refreshRecordRuleState';
import { FindingSeverityChip } from './FindingSeverityChip';
import { LoadMoreButton } from '../../components/LoadMoreButton';
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
import { Link } from '@tanstack/react-router';
import { PlayIcon } from 'lucide-react';
import { Fragment, useEffect, useRef } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { EmptyState } from '../../components/EmptyState';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RuleIcon } from '../../components/systemIcons';
import { Timestamp } from '../../time/Timestamp';
import { attributeLabel } from '../records/recordDisplay';
import { FieldRevealLink } from '../views/components/FieldRevealLink';
import { useRevealField } from '../views/fieldReveal';
import { useCan } from '../auth/useCan';
import { recheckRecord, type Finding, type RuleRun } from './api';
import {
  findingStates,
  RECORD_RULE_RUN_POLL_MS,
  ruleRoutes,
} from './constants';
import { ruleQueryKeys } from './queryKeys';
import {
  recordFindingsOptions,
  recordRuleRunsOptions,
  recordRulesOptions,
} from './queryOptions';
import { isWatchingRecordRuleChecks } from './recordRuleChecks';
import { RuleDescriptionText, RuleRevisionCell } from './RuleCells';
import { ruleDescription } from './rulePlan';
import { rulesById, type RulesById } from './rulesById';
import { findingEvidenceSchema } from './schemas';
import { capabilities } from '../auth/capabilities';
import { findingMessage, type FindingAttribute } from './findingMessage';
import { activeRunStatuses } from '../../api/runStatus';
import { progressSizes } from '../../components/iconSizes';

/** Lines up the messages beside severity chips of different lengths. */
const severityChipMinWidth = 72;
const isActive = (run: RuleRun) => activeRunStatuses.includes(run.status);

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
  attributes,
  canManage,
  canReadRules,
  contextCodes,
  finding,
  onAcknowledge,
  onRevealField,
  rules,
}: {
  acknowledging: boolean;
  attributes: readonly FindingAttribute[];
  canManage: boolean;
  /** Rule pages need `rules.read`; without it the rule is named, not linked. */
  canReadRules: boolean;
  contextCodes: readonly string[] | undefined;
  finding: Finding;
  onAcknowledge: () => void;
  /** Leads to the field the finding is about, where the page shows it. */
  onRevealField?: (code: string) => void;
  rules: RulesById;
}) => {
  const { t } = useTranslation();
  const evidence = findingEvidenceSchema.safeParse(finding.evidence);
  const contexts = evidence.success ? (evidence.data.contexts ?? []) : [];
  const everyContext =
    contextCodes !== undefined &&
    contextCodes.length > 1 &&
    contextCodes.every((code) => contexts.includes(code));
  const rule = rules.get(finding.rule_id);
  const attributeCode = evidence.success
    ? evidence.data.attribute_code
    : undefined;
  const message = findingMessage(finding.message, attributeCode, attributes);
  const attribute = attributes.find((item) => item.code === attributeCode);
  const fieldLink =
    attribute && onRevealField
      ? (label: string) => (
          <FieldRevealLink onClick={() => onRevealField(attribute.code)}>
            {label}
          </FieldRevealLink>
        )
      : undefined;
  const label = attribute && attributeLabel(attribute);
  // The field's label links where the message names it, or after it.
  const messageParts = label ? message.split(label) : [message];
  return (
    <Box
      component="li"
      sx={{
        alignItems: 'flex-start',
        display: 'flex',
        gap: 1.5,
        py: 1.5,
        '&:not(:last-of-type)': { borderBottom: 1, borderColor: 'divider' },
      }}
    >
      <FindingSeverityChip
        severity={finding.severity}
        sx={{ minWidth: severityChipMinWidth }}
      />
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography
          component="p"
          variant="body2"
          sx={{
            fontWeight: 600,
            // The rule name reads as the heading and links on hover.
            '& a': { color: 'inherit', textDecoration: 'none' },
            '& a:hover, & a:focus-visible': { textDecoration: 'underline' },
          }}
        >
          {rule && canReadRules ? (
            <Link params={{ ruleId: finding.rule_id }} to={ruleRoutes.rule}>
              {rule.name}
            </Link>
          ) : rule ? (
            rule.name
          ) : (
            <RuleRevisionCell
              rules={rules}
              ruleId={finding.rule_id}
              version={finding.rule_version}
            />
          )}
        </Typography>
        <Typography variant="body2" sx={{ color: 'text.secondary' }}>
          {fieldLink && label
            ? messageParts.map((part, index) => (
                <Fragment key={index}>
                  {index > 0 && fieldLink(label)}
                  {part}
                </Fragment>
              ))
            : message}
          {fieldLink && label && messageParts.length === 1 && (
            <>
              {' '}
              <Trans
                components={{ field: fieldLink(label) }}
                i18nKey="rules.recordFindings.aboutField"
              />
            </>
          )}
        </Typography>
        {rule && (
          <RuleDescriptionText
            description={ruleDescription(rule)}
            variant="caption"
          />
        )}
        <Typography
          component="p"
          variant="caption"
          sx={{ color: 'text.secondary' }}
        >
          {everyContext
            ? t('rules.recordFindings.allContexts')
            : contexts.length > 0 &&
              t('rules.recordFindings.contexts', {
                contexts: contexts.join(', '),
              })}
          {contexts.length > 0 && ' · '}
          <Trans
            i18nKey="rules.recordFindings.firstFound"
            components={{ timestamp: <Timestamp value={finding.created_at} /> }}
          />
        </Typography>
      </Box>
      {finding.state === findingStates.open ? (
        canManage && (
          <Button disabled={acknowledging} size="small" onClick={onAcknowledge}>
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
  attributes = [],
  contextCodes,
}: {
  recordId: string;
  blueprintId: string;
  blueprintVersion: number;
  /** The blueprint's attributes, to name the fields findings are about. */
  attributes?: readonly FindingAttribute[];
  /** Every context's code, to summarize findings that fail in all of them. */
  contextCodes?: readonly string[];
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  // Everyone who may read the record sees its findings.
  const canReadRules = useCan(capabilities.rulesRead);
  const canManage = useCan(capabilities.rulesManage);
  const revealField = useRevealField();
  const rules = useQuery(recordRulesOptions(recordId));
  const findings = useInfiniteQuery(recordFindingsOptions(recordId));
  const hasApplicableRules = (rules.data ?? []).some(
    (rule) =>
      rule.blueprint_id === blueprintId &&
      rule.enabled &&
      !rule.not_applicable.some(
        (revision) => revision.blueprint_version === blueprintVersion,
      ),
  );
  const runs = useQuery({
    ...recordRuleRunsOptions(recordId),
    // Keep checking while runs are pending or an edit's runs may still start.
    // Without an enabled rule an edit starts no runs.
    refetchInterval: (query) =>
      query.state.data?.some(isActive) ||
      (hasApplicableRules && isWatchingRecordRuleChecks(recordId))
        ? RECORD_RULE_RUN_POLL_MS
        : false,
  });
  const finishedAt = latestCompletion(runs.data);
  const seenFinish = useRef<string | null | undefined>(undefined);
  useEffect(() => {
    if (!runs.isSuccess) return;
    if (seenFinish.current !== undefined && seenFinish.current !== finishedAt)
      void Promise.all([
        client.invalidateQueries({
          queryKey: ruleQueryKeys.findings(recordId),
        }),
        refreshRecordRuleState(client, recordId),
      ]);
    seenFinish.current = finishedAt;
  }, [client, finishedAt, recordId, runs.isSuccess]);
  const recheck = useMutation({
    mutationFn: () => recheckRecord(recordId),
    onSuccess: () =>
      client.invalidateQueries({
        queryKey: ruleQueryKeys.recordRuns(recordId),
      }),
  });
  const acknowledge = useAcknowledgeFinding();

  // The server orders findings most severe first, so loading another page
  // only appends rows.
  const unresolved = findings.data?.pages.flatMap((page) => page.items) ?? [];
  const error = findings.error ?? rules.error ?? runs.error;
  const checking = Boolean(runs.data?.some(isActive)) || recheck.isPending;
  const rulesByIdMap = rulesById(rules.data ?? []);

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
              <CircularProgress
                aria-hidden
                enableTrackSlot
                size={progressSizes.compact}
              />
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
      <ApiErrorAlert error={recheck.error} sx={{ mb: 1 }} />
      <ApiErrorAlert error={acknowledge.error} sx={{ mb: 1 }} />
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
              attributes={attributes}
              canManage={canManage}
              canReadRules={canReadRules}
              contextCodes={contextCodes}
              finding={finding}
              onAcknowledge={() => acknowledge.mutate(finding.id)}
              onRevealField={revealField ?? undefined}
              rules={rulesByIdMap}
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
        <LoadMoreButton
          compact
          isLoading={findings.isFetchingNextPage}
          label={t('rules.recordFindings.loadMore')}
          onLoadMore={() => void findings.fetchNextPage()}
          sx={{ mt: 1 }}
        />
      )}
    </Box>
  );
};
