import { infiniteQueryOptions, queryOptions } from '@tanstack/react-query';
import { readAheadStaleTimeMs } from '../records/queryOptions';
import {
  getRule,
  getRuleRun,
  getUnresolvedRecordFindingCount,
  listRecordRuleRuns,
  listRecordRules,
  listRuleRevisions,
  listRuleRuns,
  listRules,
  listUnresolvedRecordFindings,
} from './api';
import { ruleQueryKeys } from './queryKeys';
import { activeRunStatuses } from '../../api/runStatus';
import { RULE_DRY_RUN_POLL_MS } from './constants';

/** The workspace's rule runs, newest first. */
export const ruleRunsOptions = () =>
  queryOptions({ queryKey: ruleQueryKeys.runs(), queryFn: listRuleRuns });

/** One run, polled until it finishes. */
export const ruleRunOptions = (runId: string) =>
  queryOptions({
    queryKey: ruleQueryKeys.run(runId),
    queryFn: () => getRuleRun(runId),
    refetchInterval: (query) =>
      !query.state.data || activeRunStatuses.includes(query.state.data.status)
        ? RULE_DRY_RUN_POLL_MS
        : false,
  });

export const ruleDefinitionsOptions = () =>
  queryOptions({
    queryKey: ruleQueryKeys.definitions(),
    queryFn: listRules,
  });

/** The rules of a record's blueprint, readable by the record's readers. */
export const recordRulesOptions = (recordId: string) =>
  queryOptions({
    queryKey: ruleQueryKeys.recordRules(recordId),
    queryFn: () => listRecordRules(recordId),
    staleTime: readAheadStaleTimeMs,
  });

export const ruleOptions = (ruleId: string) =>
  queryOptions({
    queryKey: ruleQueryKeys.rule(ruleId),
    queryFn: () => getRule(ruleId),
  });

/** A rule's saved revisions, newest first. */
export const ruleRevisionsOptions = (ruleId: string) =>
  queryOptions({
    queryKey: ruleQueryKeys.revisions(ruleId),
    queryFn: () => listRuleRevisions(ruleId),
  });

/**
 * A record's unresolved findings, page by page. The findings panel refreshes
 * them when checks finish.
 */
export const recordFindingsOptions = (recordId: string) =>
  infiniteQueryOptions({
    queryKey: ruleQueryKeys.findings(recordId),
    queryFn: ({ pageParam }) =>
      listUnresolvedRecordFindings(recordId, pageParam),
    initialPageParam: 0,
    getNextPageParam: (page) => page.nextOffset,
    staleTime: readAheadStaleTimeMs,
  });

/** How many unresolved findings a record has; refreshed with its findings. */
export const recordFindingCountOptions = (recordId: string) =>
  queryOptions({
    queryKey: ruleQueryKeys.findingCount(recordId),
    queryFn: () => getUnresolvedRecordFindingCount(recordId),
    select: (data) => data.count,
    staleTime: readAheadStaleTimeMs,
  });

/** A record's rule runs; the findings panel polls them while checks run. */
export const recordRuleRunsOptions = (recordId: string) =>
  queryOptions({
    queryKey: ruleQueryKeys.recordRuns(recordId),
    queryFn: () => listRecordRuleRuns(recordId),
    staleTime: readAheadStaleTimeMs,
  });
