import type { FindingsSearch, RunsSearch } from './schemas';

export const ruleQueryKeys = {
  all: ['rules'] as const,
  definitions: () => [...ruleQueryKeys.all, 'definitions'] as const,
  /** One rule; refreshing the definitions refreshes it. */
  rule: (ruleId: string) =>
    [...ruleQueryKeys.definitions(), 'rule', ruleId] as const,
  /** The rules of a record's blueprint; refreshing the definitions refreshes them. */
  recordRules: (recordId: string) =>
    [...ruleQueryKeys.definitions(), 'record', recordId] as const,
  /** A rule's changelog, refreshed with the rule. */
  revisions: (ruleId: string) =>
    [...ruleQueryKeys.rule(ruleId), 'revisions'] as const,
  findings: (recordId?: string) =>
    recordId
      ? ([...ruleQueryKeys.all, 'findings', recordId] as const)
      : ([...ruleQueryKeys.all, 'findings'] as const),
  /** The findings page's list; refreshing all findings refreshes it. */
  findingsPage: (search: FindingsSearch) =>
    [...ruleQueryKeys.findings(), 'page', search] as const,
  /** Counted under the record's findings, so refreshing them refreshes it. */
  findingCount: (recordId: string) =>
    [...ruleQueryKeys.findings(recordId), 'count'] as const,
  runs: () => [...ruleQueryKeys.all, 'runs'] as const,
  /** The run history's list; refreshing all runs refreshes it. */
  runsPage: (search: RunsSearch) =>
    [...ruleQueryKeys.runs(), 'page', search] as const,
  run: (runId: string) => [...ruleQueryKeys.runs(), 'run', runId] as const,
  ruleRuns: (ruleId: string) =>
    [...ruleQueryKeys.runs(), 'rule', ruleId] as const,
  recordRuns: (recordId: string) =>
    [...ruleQueryKeys.runs(), 'record', recordId] as const,
};
