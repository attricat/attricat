import { infiniteQueryOptions, queryOptions } from '@tanstack/react-query';
import { readAheadStaleTimeMs } from '../records/queryOptions';
import {
  getUnresolvedRecordFindingCount,
  listRecordRuleRuns,
  listRuleRuns,
  listRules,
  listUnresolvedRecordFindings,
} from './api';
import { ruleQueryKeys } from './queryKeys';

/** The workspace's rule runs, newest first. */
export const ruleRunsOptions = () =>
  queryOptions({ queryKey: ruleQueryKeys.runs(), queryFn: listRuleRuns });

export const ruleDefinitionsOptions = () =>
  queryOptions({
    queryKey: ruleQueryKeys.definitions(),
    queryFn: listRules,
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
