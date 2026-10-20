import { queryOptions } from '@tanstack/react-query';
import { readAheadStaleTimeMs } from '../records/queryOptions';
import { listFindings, listRecordRuleRuns, listRules } from './api';
import { ruleQueryKeys } from './queryKeys';

export const ruleDefinitionsOptions = () =>
  queryOptions({
    queryKey: ruleQueryKeys.definitions(),
    queryFn: listRules,
  });

/** A record's findings. The findings panel refreshes them when checks finish. */
export const recordFindingsOptions = (recordId: string) =>
  queryOptions({
    queryKey: ruleQueryKeys.findings(recordId),
    queryFn: () => listFindings(recordId),
    staleTime: readAheadStaleTimeMs,
  });

/** A record's rule runs; the findings panel polls them while checks run. */
export const recordRuleRunsOptions = (recordId: string) =>
  queryOptions({
    queryKey: ruleQueryKeys.recordRuns(recordId),
    queryFn: () => listRecordRuleRuns(recordId),
    staleTime: readAheadStaleTimeMs,
  });
